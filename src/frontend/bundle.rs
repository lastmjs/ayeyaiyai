#[derive(Default)]
struct ModuleLinker {
    lowerer: Lowerer,
    modules: Vec<LinkedModule>,
    module_indices: HashMap<PathBuf, usize>,
    load_order: Vec<usize>,
}

#[derive(Clone)]
struct LinkedModule {
    path: PathBuf,
    state: ModuleState,
    namespace_name: String,
    init_name: String,
    promise_name: String,
    init_async: bool,
    dependency_params: Vec<ModuleDependencyParam>,
    export_names: Vec<String>,
    export_resolutions: BTreeMap<String, ExportResolution>,
    ambiguous_export_names: HashSet<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ModuleState {
    Reserved,
    Lowering,
    Lowered,
}

#[derive(Clone)]
struct ModuleDependencyParam {
    module_index: usize,
    param_name: String,
}

#[derive(Clone)]
enum ImportBinding {
    Namespace {
        module_index: usize,
        namespace_param: String,
    },
    Named {
        module_index: usize,
        namespace_param: String,
        export_name: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ExportResolution {
    Binding {
        module_index: usize,
        binding_name: String,
        local: bool,
    },
    Namespace {
        module_index: usize,
    },
}

impl ModuleLinker {
    fn bundle_entry(&mut self, path: &Path) -> Result<Program> {
        let entry_index = self.load_module(path)?;
        self.load_order = self.compute_static_load_order(entry_index);
        let statements = self.bundle_statements(entry_index)?;
        Ok(self.lowerer.finish_program(statements, true))
    }

    fn bundle_script_entry(&mut self, path: &Path) -> Result<Program> {
        let (script, lowered_source) = parse_script_file(path)?;
        for source in collect_literal_dynamic_import_specifiers_in_statements(&script.body) {
            if let Ok(dependency_path) = resolve_module_specifier(path, &source) {
                self.load_module(&dependency_path)?;
            }
        }

        self.lowerer.source_text = Some(lowered_source);
        self.lowerer.current_module_path = Some(normalize_module_path(path)?);
        self.lowerer.module_index_lookup = self.module_indices.clone();
        let strict = script_has_use_strict_directive(&script.body);
        self.lowerer.strict_modes.push(strict);
        self.lowerer.module_mode = false;

        let mut statements = self.module_registry_statements();
        let scope_bindings = collect_direct_statement_lexical_bindings(&script.body)?;
        self.lowerer.push_binding_scope(scope_bindings);
        let lowered = self
            .lowerer
            .lower_top_level_statements(script.body.iter(), &mut statements);
        self.lowerer.pop_binding_scope();
        lowered?;

        self.lowerer.strict_modes.pop();
        self.lowerer.module_mode = false;
        self.lowerer.source_text = None;
        self.lowerer.current_module_path = None;
        self.lowerer.module_index_lookup.clear();

        Ok(self.lowerer.finish_program(statements, strict))
    }

    fn ensure_module_slot(&mut self, path: &Path) -> Result<usize> {
        let resolved = normalize_module_path(path)?;
        if let Some(index) = self.module_indices.get(&resolved).copied() {
            return Ok(index);
        }

        let module_index = self.modules.len();
        self.module_indices.insert(resolved.clone(), module_index);
        self.modules.push(LinkedModule {
            path: resolved.clone(),
            state: ModuleState::Reserved,
            namespace_name: format!("__ayy_module_namespace_{module_index}"),
            init_name: format!("__ayy_module_init_{module_index}"),
            promise_name: format!("__ayy_module_promise_{module_index}"),
            init_async: false,
            dependency_params: Vec::new(),
            export_names: Vec::new(),
            export_resolutions: BTreeMap::new(),
            ambiguous_export_names: HashSet::new(),
        });

        Ok(module_index)
    }

    fn load_module(&mut self, path: &Path) -> Result<usize> {
        let module_index = self.ensure_module_slot(path)?;
        if self.modules[module_index].state != ModuleState::Reserved {
            return Ok(module_index);
        }

        let resolved = self.modules[module_index].path.clone();
        let (module, source_text) = parse_module_file(&resolved)?;
        self.modules[module_index].state = ModuleState::Lowering;
        self.predeclare_module_export_resolutions(module_index, &module, &resolved)?;
        self.lower_module(module_index, &module, source_text)?;
        self.modules[module_index].state = ModuleState::Lowered;

        Ok(module_index)
    }

    fn lower_module(
        &mut self,
        module_index: usize,
        module: &Module,
        source_text: String,
    ) -> Result<()> {
        let exports_param = "exports".to_string();
        let module_path = self.modules[module_index].path.clone();
        ensure_module_lexical_names_are_unique(module)?;
        let module_declared_names = collect_module_declared_names(module)?;
        let mut dependency_params = Vec::new();
        let mut dependency_param_by_index = HashMap::new();
        let mut import_bindings = HashMap::new();
        let mut export_expressions = BTreeMap::<String, Expression>::new();
        let mut export_resolutions = self.modules[module_index].export_resolutions.clone();
        let mut star_export_expressions = BTreeMap::<String, Expression>::new();
        let mut star_export_resolutions = BTreeMap::<String, ExportResolution>::new();
        let mut ambiguous_star_exports = HashSet::<String>::new();
        let mut pending_self_reexports = Vec::<(String, String)>::new();
        let mut hoisted_statements = Vec::new();
        let mut body_statements = Vec::new();

        for source in collect_literal_dynamic_import_specifiers(module) {
            if let Ok(dependency_path) = resolve_module_specifier(&module_path, &source) {
                self.load_module(&dependency_path)?;
            }
        }

        for item in &module.body {
            let ModuleItem::ModuleDecl(module_declaration) = item else {
                continue;
            };
            match module_declaration {
                ModuleDecl::Import(import) => self.register_import_declaration(
                    &module_path,
                    import,
                    &mut dependency_params,
                    &mut dependency_param_by_index,
                    &mut import_bindings,
                )?,
                ModuleDecl::ExportNamed(export_named) => {
                    if let Some(source) = &export_named.src {
                        self.dependency_param_for_source(
                            &module_path,
                            &source.value.to_string_lossy(),
                            &mut dependency_params,
                            &mut dependency_param_by_index,
                        )?;
                    }
                }
                ModuleDecl::ExportAll(export_all) => {
                    self.dependency_param_for_source(
                        &module_path,
                        &export_all.src.value.to_string_lossy(),
                        &mut dependency_params,
                        &mut dependency_param_by_index,
                    )?;
                }
                _ => {}
            }
        }

        self.lowerer.strict_modes.push(true);
        self.lowerer.module_mode = true;
        self.lowerer.source_text = Some(source_text);
        self.lowerer.current_module_path = Some(module_path.clone());
        self.lowerer.module_index_lookup = self.module_indices.clone();
        let function_start = self.lowerer.functions.len();

        for item in &module.body {
            match item {
                ModuleItem::Stmt(statement) => match statement {
                    Stmt::Decl(Decl::Fn(function_declaration)) => hoisted_statements.extend(
                        self.lowerer
                            .lower_nested_function_declaration(function_declaration)?,
                    ),
                    other => {
                        body_statements.extend(self.lowerer.lower_statement(other, false, false)?)
                    }
                },
                ModuleItem::ModuleDecl(module_declaration) => match module_declaration {
                    ModuleDecl::Import(_) => {}
                    ModuleDecl::ExportDecl(export) => match &export.decl {
                        Decl::Fn(function_declaration) => {
                            hoisted_statements.extend(
                                self.lowerer
                                    .lower_nested_function_declaration(function_declaration)?,
                            );
                            let export_name = function_declaration.ident.sym.to_string();
                            export_expressions.insert(
                                export_name.clone(),
                                Expression::Identifier(export_name.clone()),
                            );
                            export_resolutions.insert(
                                export_name.clone(),
                                ExportResolution::Binding {
                                    module_index,
                                    binding_name: export_name,
                                    local: true,
                                },
                            );
                        }
                        Decl::Var(variable_declaration) => {
                            body_statements.extend(
                                self.lowerer
                                    .lower_variable_declaration(variable_declaration)?,
                            );
                            for name in collect_var_decl_bound_names(variable_declaration)? {
                                export_expressions
                                    .insert(name.clone(), Expression::Identifier(name.clone()));
                                export_resolutions.insert(
                                    name.clone(),
                                    ExportResolution::Binding {
                                        module_index,
                                        binding_name: name,
                                        local: true,
                                    },
                                );
                            }
                        }
                        Decl::Class(class_declaration) => {
                            body_statements
                                .extend(self.lowerer.lower_class_declaration(class_declaration)?);
                            let export_name = class_declaration.ident.sym.to_string();
                            export_expressions.insert(
                                export_name.clone(),
                                Expression::Identifier(export_name.clone()),
                            );
                            export_resolutions.insert(
                                export_name.clone(),
                                ExportResolution::Binding {
                                    module_index,
                                    binding_name: export_name,
                                    local: true,
                                },
                            );
                        }
                        other => bail!("unsupported export declaration: {other:?}"),
                    },
                    ModuleDecl::ExportDefaultDecl(export_default) => {
                        let expression = self.lower_default_export_declaration(
                            export_default,
                            &mut hoisted_statements,
                            &mut body_statements,
                        )?;
                        export_expressions.insert("default".to_string(), expression.clone());
                        export_resolutions.insert(
                            "default".to_string(),
                            ExportResolution::Binding {
                                module_index,
                                binding_name: match expression {
                                    Expression::Identifier(name) => name,
                                    _ => "default".to_string(),
                                },
                                local: true,
                            },
                        );
                    }
                    ModuleDecl::ExportDefaultExpr(export_default) => {
                        let local_name = self.lowerer.fresh_temporary_name("module_default");
                        body_statements.push(Statement::Let {
                            name: local_name.clone(),
                            mutable: false,
                            value: self.lowerer.lower_expression_with_name_hint(
                                &export_default.expr,
                                Some("default"),
                            )?,
                        });
                        export_expressions.insert(
                            "default".to_string(),
                            Expression::Identifier(local_name.clone()),
                        );
                        export_resolutions.insert(
                            "default".to_string(),
                            ExportResolution::Binding {
                                module_index,
                                binding_name: local_name,
                                local: true,
                            },
                        );
                    }
                    ModuleDecl::ExportNamed(export_named) if export_named.src.is_none() => {
                        for specifier in &export_named.specifiers {
                            match specifier {
                                ExportSpecifier::Named(named) => {
                                    let local_name = module_export_name_string(&named.orig)?;
                                    ensure!(
                                        import_bindings.contains_key(&local_name)
                                            || module_declared_names.contains(&local_name),
                                        "unresolvable export `{local_name}`"
                                    );
                                    let export_name = named
                                        .exported
                                        .as_ref()
                                        .map(module_export_name_string)
                                        .transpose()?
                                        .unwrap_or_else(|| local_name.clone());
                                    export_expressions.insert(
                                        export_name.clone(),
                                        Expression::Identifier(local_name.clone()),
                                    );
                                    let resolution = import_bindings
                                        .get(&local_name)
                                        .map(|binding| {
                                            self.export_resolution_for_import_binding(binding)
                                        })
                                        .transpose()?
                                        .unwrap_or_else(|| ExportResolution::Binding {
                                            module_index,
                                            binding_name: local_name,
                                            local: true,
                                        });
                                    export_resolutions.insert(export_name, resolution);
                                }
                                other => bail!("unsupported local export specifier: {other:?}"),
                            }
                        }
                    }
                    ModuleDecl::ExportNamed(export_named) => {
                        let source = export_named
                            .src
                            .as_ref()
                            .context("re-export must have a source")?;
                        let namespace_param = self.dependency_param_for_source(
                            &module_path,
                            &source.value.to_string_lossy(),
                            &mut dependency_params,
                            &mut dependency_param_by_index,
                        )?;
                        let dependency_index = dependency_params
                            .iter()
                            .find(|dependency| dependency.param_name == namespace_param)
                            .map(|dependency| dependency.module_index)
                            .context("re-export dependency must be registered")?;
                        let dependency_finalized = self.load_order.contains(&dependency_index);
                        let self_reexport = dependency_index == module_index;
                        for specifier in &export_named.specifiers {
                            match specifier {
                                ExportSpecifier::Named(named) => {
                                    let imported_name = module_export_name_string(&named.orig)?;
                                    let export_name = named
                                        .exported
                                        .as_ref()
                                        .map(module_export_name_string)
                                        .transpose()?
                                        .unwrap_or_else(|| imported_name.clone());
                                    export_expressions.insert(
                                        export_name.clone(),
                                        Expression::Member {
                                            object: Box::new(Expression::Identifier(
                                                namespace_param.clone(),
                                            )),
                                            property: Box::new(Expression::String(
                                                imported_name.clone(),
                                            )),
                                        },
                                    );
                                    if self_reexport {
                                        pending_self_reexports.push((export_name, imported_name));
                                    } else if !dependency_finalized {
                                        export_resolutions.insert(
                                            export_name,
                                            ExportResolution::Binding {
                                                module_index: dependency_index,
                                                binding_name: imported_name,
                                                local: false,
                                            },
                                        );
                                    } else {
                                        export_resolutions.insert(
                                            export_name,
                                            self.require_export_resolution_for_dependency(
                                                dependency_index,
                                                &imported_name,
                                            )?,
                                        );
                                    }
                                }
                                ExportSpecifier::Namespace(namespace) => {
                                    let export_name = module_export_name_string(&namespace.name)?;
                                    export_expressions.insert(
                                        export_name.clone(),
                                        Expression::Identifier(namespace_param.clone()),
                                    );
                                    export_resolutions.insert(
                                        export_name,
                                        ExportResolution::Namespace {
                                            module_index: dependency_index,
                                        },
                                    );
                                }
                                ExportSpecifier::Default(default) => {
                                    let export_name = default.exported.sym.to_string();
                                    export_expressions.insert(
                                        export_name.clone(),
                                        Expression::Member {
                                            object: Box::new(Expression::Identifier(
                                                namespace_param.clone(),
                                            )),
                                            property: Box::new(Expression::String(
                                                "default".to_string(),
                                            )),
                                        },
                                    );
                                    if self_reexport {
                                        pending_self_reexports
                                            .push((export_name, "default".to_string()));
                                    } else if !dependency_finalized {
                                        export_resolutions.insert(
                                            export_name,
                                            ExportResolution::Binding {
                                                module_index: dependency_index,
                                                binding_name: "default".to_string(),
                                                local: false,
                                            },
                                        );
                                    } else {
                                        export_resolutions.insert(
                                            export_name,
                                            self.require_export_resolution_for_dependency(
                                                dependency_index,
                                                "default",
                                            )?,
                                        );
                                    }
                                }
                            }
                        }
                    }
                    ModuleDecl::ExportAll(export_all) => {
                        let namespace_param = self.dependency_param_for_source(
                            &module_path,
                            &export_all.src.value.to_string_lossy(),
                            &mut dependency_params,
                            &mut dependency_param_by_index,
                        )?;
                        let dependency_index = dependency_params
                            .iter()
                            .find(|dependency| dependency.param_name == namespace_param)
                            .map(|dependency| dependency.module_index)
                            .context("export-all dependency must be registered")?;
                        let dependency_export_resolutions =
                            self.modules[dependency_index].export_resolutions.clone();
                        for (export_name, resolution) in dependency_export_resolutions {
                            if export_name == "default" {
                                continue;
                            }
                            if export_resolutions.contains_key(&export_name)
                                || ambiguous_star_exports.contains(&export_name)
                            {
                                continue;
                            }

                            let expression = Expression::Member {
                                object: Box::new(Expression::Identifier(namespace_param.clone())),
                                property: Box::new(Expression::String(export_name.clone())),
                            };
                            if let Some(previous_resolution) =
                                star_export_resolutions.get(&export_name)
                            {
                                if previous_resolution != &resolution {
                                    star_export_expressions.remove(&export_name);
                                    star_export_resolutions.remove(&export_name);
                                    ambiguous_star_exports.insert(export_name);
                                }
                            } else {
                                star_export_expressions.insert(export_name.clone(), expression);
                                star_export_resolutions.insert(export_name, resolution);
                            }
                        }
                    }
                    other => bail!("unsupported module declaration: {other:?}"),
                },
            }
        }

        for (export_name, expression) in star_export_expressions {
            if !export_expressions.contains_key(&export_name) {
                export_expressions.insert(export_name, expression);
            }
        }
        for (export_name, resolution) in star_export_resolutions {
            if !export_resolutions.contains_key(&export_name) {
                export_resolutions.insert(export_name, resolution);
            }
        }
        for (export_name, imported_name) in pending_self_reexports {
            if let Some(resolution) = export_resolutions.get(&imported_name).cloned() {
                export_resolutions.insert(export_name, resolution);
            } else if let Some(Expression::Identifier(binding_name)) =
                export_expressions.get(&imported_name)
            {
                export_resolutions.insert(
                    export_name,
                    ExportResolution::Binding {
                        module_index,
                        binding_name: binding_name.clone(),
                        local: true,
                    },
                );
            } else if ambiguous_star_exports.contains(&imported_name) {
                bail!(
                    "ambiguous export `{imported_name}` in `{}`",
                    self.modules[module_index].path.display()
                );
            } else {
                bail!(
                    "missing export `{imported_name}` in `{}`",
                    self.modules[module_index].path.display()
                );
            }
        }

        self.lowerer.strict_modes.pop();
        self.lowerer.module_mode = false;
        self.lowerer.source_text = None;
        self.lowerer.current_module_path = None;
        self.lowerer.module_index_lookup.clear();

        self.rewrite_import_bindings_in_statements(&mut hoisted_statements, &import_bindings)?;
        self.rewrite_import_bindings_in_statements(&mut body_statements, &import_bindings)?;
        for function in &mut self.lowerer.functions[function_start..] {
            rewrite_import_bindings_in_function(function, &import_bindings)?;
        }

        let mut init_body = self.build_module_namespace_prelude(&exports_param);
        init_body.extend(self.build_export_getter_statements(
            module_index,
            &exports_param,
            &export_expressions,
            &import_bindings,
        )?);
        init_body.extend(hoisted_statements);
        init_body.extend(body_statements);
        let (init_body, init_async) = asyncify_statements(init_body);

        let mut params = vec![Parameter {
            name: exports_param,
            default: None,
            rest: false,
        }];
        params.extend(dependency_params.iter().map(|dependency| Parameter {
            name: dependency.param_name.clone(),
            default: None,
            rest: false,
        }));

        self.lowerer.functions.push(FunctionDeclaration {
            name: self.modules[module_index].init_name.clone(),
            top_level_binding: None,
            params,
            body: init_body,
            register_global: false,
            kind: if init_async {
                FunctionKind::Async
            } else {
                FunctionKind::Ordinary
            },
            self_binding: None,
            mapped_arguments: false,
            strict: true,
            lexical_this: false,
            length: dependency_params.len() + 1,
        });

        self.modules[module_index].init_async = init_async;
        self.modules[module_index].dependency_params = dependency_params;
        self.modules[module_index].export_names = export_expressions.keys().cloned().collect();
        self.modules[module_index].export_resolutions = export_resolutions;
        self.modules[module_index].ambiguous_export_names = ambiguous_star_exports;

        Ok(())
    }

    fn compute_static_load_order(&self, entry_index: usize) -> Vec<usize> {
        fn visit(
            linker: &ModuleLinker,
            module_index: usize,
            visited: &mut HashSet<usize>,
            order: &mut Vec<usize>,
        ) {
            if !visited.insert(module_index) {
                return;
            }

            for dependency in &linker.modules[module_index].dependency_params {
                if dependency.module_index != module_index {
                    visit(linker, dependency.module_index, visited, order);
                }
            }

            order.push(module_index);
        }

        let mut order = Vec::new();
        visit(self, entry_index, &mut HashSet::new(), &mut order);
        order
    }

    fn predeclare_module_export_resolutions(
        &mut self,
        module_index: usize,
        module: &Module,
        module_path: &Path,
    ) -> Result<()> {
        let module_declared_names = collect_module_declared_names(module)?;
        let mut export_resolutions = BTreeMap::new();
        for item in &module.body {
            let ModuleItem::ModuleDecl(module_declaration) = item else {
                continue;
            };
            match module_declaration {
                ModuleDecl::ExportDecl(export) => match &export.decl {
                    Decl::Fn(function_declaration) => {
                        let export_name = function_declaration.ident.sym.to_string();
                        export_resolutions.insert(
                            export_name.clone(),
                            ExportResolution::Binding {
                                module_index,
                                binding_name: export_name,
                                local: true,
                            },
                        );
                    }
                    Decl::Var(variable_declaration) => {
                        for name in collect_var_decl_bound_names(variable_declaration)? {
                            export_resolutions.insert(
                                name.clone(),
                                ExportResolution::Binding {
                                    module_index,
                                    binding_name: name,
                                    local: true,
                                },
                            );
                        }
                    }
                    Decl::Class(class_declaration) => {
                        let export_name = class_declaration.ident.sym.to_string();
                        export_resolutions.insert(
                            export_name.clone(),
                            ExportResolution::Binding {
                                module_index,
                                binding_name: export_name,
                                local: true,
                            },
                        );
                    }
                    _ => {}
                },
                ModuleDecl::ExportDefaultDecl(export_default) => {
                    let binding_name = match &export_default.decl {
                        DefaultDecl::Class(class_expression) => class_expression
                            .ident
                            .as_ref()
                            .map(|identifier| identifier.sym.to_string())
                            .unwrap_or_else(|| "default".to_string()),
                        DefaultDecl::Fn(function_expression) => function_expression
                            .ident
                            .as_ref()
                            .map(|identifier| identifier.sym.to_string())
                            .unwrap_or_else(|| "default".to_string()),
                        _ => "default".to_string(),
                    };
                    export_resolutions.insert(
                        "default".to_string(),
                        ExportResolution::Binding {
                            module_index,
                            binding_name,
                            local: true,
                        },
                    );
                }
                ModuleDecl::ExportDefaultExpr(_) => {
                    export_resolutions.insert(
                        "default".to_string(),
                        ExportResolution::Binding {
                            module_index,
                            binding_name: "default".to_string(),
                            local: true,
                        },
                    );
                }
                ModuleDecl::ExportNamed(export_named) if export_named.src.is_none() => {
                    for specifier in &export_named.specifiers {
                        let ExportSpecifier::Named(named) = specifier else {
                            continue;
                        };
                        let local_name = module_export_name_string(&named.orig)?;
                        if !module_declared_names.contains(&local_name) {
                            continue;
                        }
                        let export_name = named
                            .exported
                            .as_ref()
                            .map(module_export_name_string)
                            .transpose()?
                            .unwrap_or_else(|| local_name.clone());
                        export_resolutions.insert(
                            export_name,
                            ExportResolution::Binding {
                                module_index,
                                binding_name: local_name,
                                local: true,
                            },
                        );
                    }
                }
                ModuleDecl::ExportNamed(export_named) => {
                    let source = export_named
                        .src
                        .as_ref()
                        .context("re-export must have a source")?;
                    let dependency_path =
                        resolve_module_specifier(module_path, &source.value.to_string_lossy())?;
                    let dependency_index = self.ensure_module_slot(&dependency_path)?;
                    for specifier in &export_named.specifiers {
                        match specifier {
                            ExportSpecifier::Named(named) => {
                                let imported_name = module_export_name_string(&named.orig)?;
                                let export_name = named
                                    .exported
                                    .as_ref()
                                    .map(module_export_name_string)
                                    .transpose()?
                                    .unwrap_or_else(|| imported_name.clone());
                                export_resolutions.insert(
                                    export_name,
                                    ExportResolution::Binding {
                                        module_index: dependency_index,
                                        binding_name: imported_name,
                                        local: false,
                                    },
                                );
                            }
                            ExportSpecifier::Namespace(namespace) => {
                                let export_name = module_export_name_string(&namespace.name)?;
                                export_resolutions.insert(
                                    export_name,
                                    ExportResolution::Namespace {
                                        module_index: dependency_index,
                                    },
                                );
                            }
                            ExportSpecifier::Default(default) => {
                                export_resolutions.insert(
                                    default.exported.sym.to_string(),
                                    ExportResolution::Binding {
                                        module_index: dependency_index,
                                        binding_name: "default".to_string(),
                                        local: false,
                                    },
                                );
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        self.modules[module_index].export_resolutions = export_resolutions;
        Ok(())
    }

    fn require_export_resolution_for_dependency(
        &self,
        module_index: usize,
        export_name: &str,
    ) -> Result<ExportResolution> {
        self.require_export_resolution_for_dependency_with_visited(
            module_index,
            export_name,
            &mut HashSet::new(),
        )
    }

    fn require_export_resolution_for_dependency_with_visited(
        &self,
        module_index: usize,
        export_name: &str,
        visited: &mut HashSet<(usize, String)>,
    ) -> Result<ExportResolution> {
        if !visited.insert((module_index, export_name.to_string())) {
            bail!(
                "circular indirect export `{export_name}` in `{}`",
                self.modules[module_index].path.display()
            );
        }

        if let Some(resolution) = self.modules[module_index]
            .export_resolutions
            .get(export_name)
            .cloned()
        {
            return match resolution {
                ExportResolution::Binding {
                    module_index,
                    binding_name,
                    local: true,
                } => Ok(ExportResolution::Binding {
                    module_index,
                    binding_name,
                    local: true,
                }),
                ExportResolution::Binding {
                    module_index,
                    binding_name,
                    local: false,
                } => self.require_export_resolution_for_dependency_with_visited(
                    module_index,
                    &binding_name,
                    visited,
                ),
                ExportResolution::Namespace { module_index } => {
                    Ok(ExportResolution::Namespace { module_index })
                }
            };
        }

        if self.modules[module_index]
            .ambiguous_export_names
            .contains(export_name)
        {
            bail!(
                "ambiguous export `{export_name}` in `{}`",
                self.modules[module_index].path.display()
            );
        }

        bail!(
            "missing export `{export_name}` in `{}`",
            self.modules[module_index].path.display()
        );
    }

    fn export_resolution_for_import_binding(
        &self,
        binding: &ImportBinding,
    ) -> Result<ExportResolution> {
        match binding {
            ImportBinding::Namespace { module_index, .. } => Ok(ExportResolution::Namespace {
                module_index: *module_index,
            }),
            ImportBinding::Named {
                module_index,
                export_name,
                ..
            } => self.require_export_resolution_for_dependency(*module_index, export_name),
        }
    }

    fn register_import_declaration(
        &mut self,
        module_path: &Path,
        import: &ImportDecl,
        dependency_params: &mut Vec<ModuleDependencyParam>,
        dependency_param_by_index: &mut HashMap<usize, String>,
        import_bindings: &mut HashMap<String, ImportBinding>,
    ) -> Result<()> {
        ensure!(!import.type_only, "type-only imports are not supported yet");
        validate_import_attributes(import.with.as_deref())?;
        let namespace_param = self.dependency_param_for_source(
            module_path,
            &import.src.value.to_string_lossy(),
            dependency_params,
            dependency_param_by_index,
        )?;
        let dependency_index = dependency_params
            .iter()
            .find(|dependency| dependency.param_name == namespace_param)
            .map(|dependency| dependency.module_index)
            .context("import dependency must be registered")?;

        for specifier in &import.specifiers {
            match specifier {
                ImportSpecifier::Named(named) => {
                    ensure!(
                        !named.is_type_only,
                        "type-only named imports are not supported yet"
                    );
                    let export_name = named
                        .imported
                        .as_ref()
                        .map(module_export_name_string)
                        .transpose()?
                        .unwrap_or_else(|| named.local.sym.to_string());
                    self.require_export_resolution_for_dependency(dependency_index, &export_name)?;
                    import_bindings.insert(
                        named.local.sym.to_string(),
                        ImportBinding::Named {
                            module_index: dependency_index,
                            namespace_param: namespace_param.clone(),
                            export_name,
                        },
                    );
                }
                ImportSpecifier::Default(default) => {
                    self.require_export_resolution_for_dependency(dependency_index, "default")?;
                    import_bindings.insert(
                        default.local.sym.to_string(),
                        ImportBinding::Named {
                            module_index: dependency_index,
                            namespace_param: namespace_param.clone(),
                            export_name: "default".to_string(),
                        },
                    );
                }
                ImportSpecifier::Namespace(namespace) => {
                    import_bindings.insert(
                        namespace.local.sym.to_string(),
                        ImportBinding::Namespace {
                            module_index: dependency_index,
                            namespace_param: namespace_param.clone(),
                        },
                    );
                }
            }
        }

        Ok(())
    }

    fn dependency_param_for_source(
        &mut self,
        module_path: &Path,
        source: &str,
        dependency_params: &mut Vec<ModuleDependencyParam>,
        dependency_param_by_index: &mut HashMap<usize, String>,
    ) -> Result<String> {
        let dependency_path = resolve_module_specifier(module_path, source)?;
        let dependency_index = self.load_module(&dependency_path)?;
        if let Some(existing) = dependency_param_by_index.get(&dependency_index) {
            return Ok(existing.clone());
        }

        let param_name = self.lowerer.fresh_temporary_name("module_dep");
        dependency_param_by_index.insert(dependency_index, param_name.clone());
        dependency_params.push(ModuleDependencyParam {
            module_index: dependency_index,
            param_name: param_name.clone(),
        });
        Ok(param_name)
    }

    fn lower_default_export_declaration(
        &mut self,
        export_default: &ExportDefaultDecl,
        hoisted_statements: &mut Vec<Statement>,
        body_statements: &mut Vec<Statement>,
    ) -> Result<Expression> {
        match &export_default.decl {
            DefaultDecl::Fn(function_expression) => {
                if let Some(identifier) = &function_expression.ident {
                    let generated_name = self
                        .lowerer
                        .lower_named_default_function_expression(function_expression)?;
                    hoisted_statements.push(Statement::Let {
                        name: identifier.sym.to_string(),
                        mutable: true,
                        value: Expression::Identifier(generated_name),
                    });
                    Ok(Expression::Identifier(identifier.sym.to_string()))
                } else {
                    let local_name = self.lowerer.fresh_temporary_name("module_default");
                    hoisted_statements.push(Statement::Let {
                        name: local_name.clone(),
                        mutable: true,
                        value: self
                            .lowerer
                            .lower_function_expression(function_expression, Some("default"))?,
                    });
                    Ok(Expression::Identifier(local_name))
                }
            }
            DefaultDecl::Class(class_expression) => {
                let local_name = class_expression
                    .ident
                    .as_ref()
                    .map(|identifier| identifier.sym.to_string())
                    .unwrap_or_else(|| "default".to_string());
                body_statements.extend(
                    self.lowerer
                        .lower_class_definition(&class_expression.class, local_name.clone())?,
                );
                Ok(Expression::Identifier(local_name))
            }
            other => bail!("unsupported default export declaration: {other:?}"),
        }
    }

    fn build_module_namespace_prelude(&self, exports_param: &str) -> Vec<Statement> {
        vec![
            define_property_statement(
                Expression::Identifier(exports_param.to_string()),
                Expression::Member {
                    object: Box::new(Expression::Identifier("Symbol".to_string())),
                    property: Box::new(Expression::String("toStringTag".to_string())),
                },
                Expression::Object(vec![
                    ObjectEntry::Data {
                        key: Expression::String("value".to_string()),
                        value: Expression::String("Module".to_string()),
                    },
                    ObjectEntry::Data {
                        key: Expression::String("writable".to_string()),
                        value: Expression::Bool(false),
                    },
                    ObjectEntry::Data {
                        key: Expression::String("enumerable".to_string()),
                        value: Expression::Bool(false),
                    },
                    ObjectEntry::Data {
                        key: Expression::String("configurable".to_string()),
                        value: Expression::Bool(false),
                    },
                ]),
            ),
            define_property_statement(
                Expression::Identifier(exports_param.to_string()),
                Expression::String("__ayy$module$namespace".to_string()),
                data_property_descriptor(Expression::Bool(true), false, false, false),
            ),
        ]
    }

    fn build_export_getter_statements(
        &mut self,
        module_index: usize,
        exports_param: &str,
        export_expressions: &BTreeMap<String, Expression>,
        import_bindings: &HashMap<String, ImportBinding>,
    ) -> Result<Vec<Statement>> {
        let mut statements = Vec::new();

        for (export_name, expression) in export_expressions {
            let getter_name = format!(
                "__ayy_module_export_getter_{}_{}",
                module_index,
                self.lowerer.fresh_temporary_name("getter")
            );
            let mut getter_function = FunctionDeclaration {
                name: getter_name.clone(),
                top_level_binding: None,
                params: Vec::new(),
                body: vec![Statement::Return(expression.clone())],
                register_global: false,
                kind: FunctionKind::Ordinary,
                self_binding: None,
                mapped_arguments: false,
                strict: true,
                lexical_this: false,
                length: 0,
            };
            rewrite_import_bindings_in_function(&mut getter_function, import_bindings)?;
            self.lowerer.functions.push(getter_function);

            statements.push(define_property_statement(
                Expression::Identifier(exports_param.to_string()),
                Expression::String(export_name.clone()),
                Expression::Object(vec![
                    ObjectEntry::Data {
                        key: Expression::String("get".to_string()),
                        value: Expression::Identifier(getter_name),
                    },
                    ObjectEntry::Data {
                        key: Expression::String("enumerable".to_string()),
                        value: Expression::Bool(true),
                    },
                    ObjectEntry::Data {
                        key: Expression::String("configurable".to_string()),
                        value: Expression::Bool(false),
                    },
                ]),
            ));
        }

        Ok(statements)
    }

    fn module_registry_statements(&self) -> Vec<Statement> {
        let mut statements = Vec::new();

        for module in &self.modules {
            statements.push(Statement::Let {
                name: module.namespace_name.clone(),
                mutable: false,
                value: Expression::Call {
                    callee: Box::new(Expression::Member {
                        object: Box::new(Expression::Identifier("Object".to_string())),
                        property: Box::new(Expression::String("create".to_string())),
                    }),
                    arguments: vec![CallArgument::Expression(Expression::Null)],
                },
            });
        }

        statements
    }

    fn module_init_call_arguments(&self, module_index: usize) -> Vec<CallArgument> {
        let module = &self.modules[module_index];
        let mut arguments = vec![CallArgument::Expression(Expression::Identifier(
            module.namespace_name.clone(),
        ))];
        for dependency in &module.dependency_params {
            arguments.push(CallArgument::Expression(Expression::Identifier(
                self.modules[dependency.module_index].namespace_name.clone(),
            )));
        }
        arguments
    }

    fn bundle_statements(&self, entry_index: usize) -> Result<Vec<Statement>> {
        let mut statements = self.module_registry_statements();

        for &module_index in &self.load_order {
            let module = &self.modules[module_index];
            statements.push(Statement::Let {
                name: module.promise_name.clone(),
                mutable: false,
                value: Expression::Call {
                    callee: Box::new(Expression::Identifier(module.init_name.clone())),
                    arguments: self.module_init_call_arguments(module_index),
                },
            });
        }

        statements.push(Statement::Expression(Expression::Await(Box::new(
            Expression::Identifier(self.modules[entry_index].promise_name.clone()),
        ))));

        Ok(statements)
    }

    fn rewrite_import_bindings_in_statements(
        &self,
        statements: &mut [Statement],
        import_bindings: &HashMap<String, ImportBinding>,
    ) -> Result<()> {
        let mut rewriter = ImportBindingRewriter::new(import_bindings);
        rewriter.rewrite_statement_list(statements)
    }
}

fn normalize_module_path(path: &Path) -> Result<PathBuf> {
    path.canonicalize()
        .with_context(|| format!("failed to resolve module path `{}`", path.display()))
}

fn resolve_module_specifier(module_path: &Path, source: &str) -> Result<PathBuf> {
    ensure!(
        source.starts_with("./") || source.starts_with("../") || source.starts_with('/'),
        "unsupported module specifier `{source}`"
    );
    let candidate = if source.starts_with('/') {
        PathBuf::from(source)
    } else {
        module_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(source)
    };
    normalize_module_path(&candidate)
}

fn collect_literal_dynamic_import_specifiers(module: &Module) -> Vec<String> {
    let mut specifiers = Vec::new();
    let mut seen = HashSet::new();

    for item in &module.body {
        collect_dynamic_imports_from_module_item(item, &mut specifiers, &mut seen);
    }

    specifiers
}

fn collect_literal_dynamic_import_specifiers_in_statements(statements: &[Stmt]) -> Vec<String> {
    let mut specifiers = Vec::new();
    let mut seen = HashSet::new();

    for statement in statements {
        collect_dynamic_imports_from_statement(statement, &mut specifiers, &mut seen);
    }

    specifiers
}

fn collect_dynamic_imports_from_module_item(
    item: &ModuleItem,
    specifiers: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    match item {
        ModuleItem::Stmt(statement) => {
            collect_dynamic_imports_from_statement(statement, specifiers, seen)
        }
        ModuleItem::ModuleDecl(declaration) => {
            collect_dynamic_imports_from_module_declaration(declaration, specifiers, seen)
        }
    }
}

fn collect_dynamic_imports_from_module_declaration(
    declaration: &ModuleDecl,
    specifiers: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    match declaration {
        ModuleDecl::ExportDecl(export) => {
            collect_dynamic_imports_from_declaration(&export.decl, specifiers, seen);
        }
        ModuleDecl::ExportDefaultDecl(export_default) => match &export_default.decl {
            DefaultDecl::Fn(function) => {
                collect_dynamic_imports_from_function(&function.function, specifiers, seen)
            }
            DefaultDecl::Class(class) => {
                collect_dynamic_imports_from_class(&class.class, specifiers, seen)
            }
            _ => {}
        },
        ModuleDecl::ExportDefaultExpr(export_default) => {
            collect_dynamic_imports_from_expression(&export_default.expr, specifiers, seen);
        }
        ModuleDecl::ExportNamed(_) | ModuleDecl::ExportAll(_) => {}
        _ => {}
    }
}

fn collect_dynamic_imports_from_statement(
    statement: &Stmt,
    specifiers: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    match statement {
        Stmt::Block(block) => {
            for statement in &block.stmts {
                collect_dynamic_imports_from_statement(statement, specifiers, seen);
            }
        }
        Stmt::Decl(declaration) => {
            collect_dynamic_imports_from_declaration(declaration, specifiers, seen)
        }
        Stmt::Expr(expression) => {
            collect_dynamic_imports_from_expression(&expression.expr, specifiers, seen)
        }
        Stmt::If(statement) => {
            collect_dynamic_imports_from_expression(&statement.test, specifiers, seen);
            collect_dynamic_imports_from_statement(&statement.cons, specifiers, seen);
            if let Some(alternate) = &statement.alt {
                collect_dynamic_imports_from_statement(alternate, specifiers, seen);
            }
        }
        Stmt::While(statement) => {
            collect_dynamic_imports_from_expression(&statement.test, specifiers, seen);
            collect_dynamic_imports_from_statement(&statement.body, specifiers, seen);
        }
        Stmt::DoWhile(statement) => {
            collect_dynamic_imports_from_statement(&statement.body, specifiers, seen);
            collect_dynamic_imports_from_expression(&statement.test, specifiers, seen);
        }
        Stmt::For(statement) => {
            if let Some(init) = &statement.init {
                match init {
                    VarDeclOrExpr::VarDecl(variable_declaration) => {
                        collect_dynamic_imports_from_variable_declaration(
                            variable_declaration,
                            specifiers,
                            seen,
                        );
                    }
                    VarDeclOrExpr::Expr(expression) => {
                        collect_dynamic_imports_from_expression(expression, specifiers, seen);
                    }
                }
            }
            if let Some(test) = &statement.test {
                collect_dynamic_imports_from_expression(test, specifiers, seen);
            }
            if let Some(update) = &statement.update {
                collect_dynamic_imports_from_expression(update, specifiers, seen);
            }
            collect_dynamic_imports_from_statement(&statement.body, specifiers, seen);
        }
        Stmt::ForIn(statement) => {
            collect_dynamic_imports_from_for_head(&statement.left, specifiers, seen);
            collect_dynamic_imports_from_expression(&statement.right, specifiers, seen);
            collect_dynamic_imports_from_statement(&statement.body, specifiers, seen);
        }
        Stmt::ForOf(statement) => {
            collect_dynamic_imports_from_for_head(&statement.left, specifiers, seen);
            collect_dynamic_imports_from_expression(&statement.right, specifiers, seen);
            collect_dynamic_imports_from_statement(&statement.body, specifiers, seen);
        }
        Stmt::Switch(statement) => {
            collect_dynamic_imports_from_expression(&statement.discriminant, specifiers, seen);
            for case in &statement.cases {
                if let Some(test) = &case.test {
                    collect_dynamic_imports_from_expression(test, specifiers, seen);
                }
                for statement in &case.cons {
                    collect_dynamic_imports_from_statement(statement, specifiers, seen);
                }
            }
        }
        Stmt::Try(statement) => {
            for statement in &statement.block.stmts {
                collect_dynamic_imports_from_statement(statement, specifiers, seen);
            }
            if let Some(handler) = &statement.handler {
                if let Some(pattern) = &handler.param {
                    collect_dynamic_imports_from_pattern(pattern, specifiers, seen);
                }
                for statement in &handler.body.stmts {
                    collect_dynamic_imports_from_statement(statement, specifiers, seen);
                }
            }
            if let Some(finalizer) = &statement.finalizer {
                for statement in &finalizer.stmts {
                    collect_dynamic_imports_from_statement(statement, specifiers, seen);
                }
            }
        }
        Stmt::With(statement) => {
            collect_dynamic_imports_from_expression(&statement.obj, specifiers, seen);
            collect_dynamic_imports_from_statement(&statement.body, specifiers, seen);
        }
        Stmt::Return(statement) => {
            if let Some(argument) = &statement.arg {
                collect_dynamic_imports_from_expression(argument, specifiers, seen);
            }
        }
        Stmt::Throw(statement) => {
            collect_dynamic_imports_from_expression(&statement.arg, specifiers, seen);
        }
        Stmt::Labeled(statement) => {
            collect_dynamic_imports_from_statement(&statement.body, specifiers, seen)
        }
        _ => {}
    }
}

fn collect_dynamic_imports_from_declaration(
    declaration: &Decl,
    specifiers: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    match declaration {
        Decl::Fn(function) => {
            collect_dynamic_imports_from_function(&function.function, specifiers, seen)
        }
        Decl::Class(class) => collect_dynamic_imports_from_class(&class.class, specifiers, seen),
        Decl::Var(variable_declaration) => collect_dynamic_imports_from_variable_declaration(
            variable_declaration,
            specifiers,
            seen,
        ),
        _ => {}
    }
}

fn collect_dynamic_imports_from_variable_declaration(
    declaration: &swc_ecma_ast::VarDecl,
    specifiers: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    for declarator in &declaration.decls {
        collect_dynamic_imports_from_pattern(&declarator.name, specifiers, seen);
        if let Some(initializer) = &declarator.init {
            collect_dynamic_imports_from_expression(initializer, specifiers, seen);
        }
    }
}

fn collect_dynamic_imports_from_for_head(
    head: &ForHead,
    specifiers: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    match head {
        ForHead::VarDecl(variable_declaration) => {
            collect_dynamic_imports_from_variable_declaration(
                variable_declaration,
                specifiers,
                seen,
            )
        }
        ForHead::Pat(pattern) => collect_dynamic_imports_from_pattern(pattern, specifiers, seen),
        ForHead::UsingDecl(_) => {}
    }
}

fn collect_dynamic_imports_from_pattern(
    pattern: &Pat,
    specifiers: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    match pattern {
        Pat::Assign(assign) => {
            collect_dynamic_imports_from_pattern(&assign.left, specifiers, seen);
            collect_dynamic_imports_from_expression(&assign.right, specifiers, seen);
        }
        Pat::Array(array) => {
            for element in array.elems.iter().flatten() {
                collect_dynamic_imports_from_pattern(element, specifiers, seen);
            }
        }
        Pat::Object(object) => {
            for property in &object.props {
                match property {
                    ObjectPatProp::KeyValue(property) => {
                        collect_dynamic_imports_from_pattern(&property.value, specifiers, seen);
                    }
                    ObjectPatProp::Assign(property) => {
                        if let Some(value) = &property.value {
                            collect_dynamic_imports_from_expression(value, specifiers, seen);
                        }
                    }
                    ObjectPatProp::Rest(rest) => {
                        collect_dynamic_imports_from_pattern(&rest.arg, specifiers, seen);
                    }
                }
            }
        }
        Pat::Rest(rest) => collect_dynamic_imports_from_pattern(&rest.arg, specifiers, seen),
        _ => {}
    }
}

fn collect_dynamic_imports_from_function(
    function: &Function,
    specifiers: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    for parameter in &function.params {
        collect_dynamic_imports_from_pattern(&parameter.pat, specifiers, seen);
    }
    if let Some(body) = &function.body {
        for statement in &body.stmts {
            collect_dynamic_imports_from_statement(statement, specifiers, seen);
        }
    }
}

fn collect_dynamic_imports_from_class(
    class: &Class,
    specifiers: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    if let Some(super_class) = &class.super_class {
        collect_dynamic_imports_from_expression(super_class, specifiers, seen);
    }

    for member in &class.body {
        match member {
            ClassMember::Constructor(constructor) => {
                if let Some(body) = &constructor.body {
                    for statement in &body.stmts {
                        collect_dynamic_imports_from_statement(statement, specifiers, seen);
                    }
                }
            }
            ClassMember::Method(method) => {
                collect_dynamic_imports_from_property_name(&method.key, specifiers, seen);
                collect_dynamic_imports_from_function(&method.function, specifiers, seen);
            }
            ClassMember::ClassProp(property) => {
                collect_dynamic_imports_from_property_name(&property.key, specifiers, seen);
                if let Some(value) = &property.value {
                    collect_dynamic_imports_from_expression(value, specifiers, seen);
                }
            }
            ClassMember::PrivateMethod(method) => {
                collect_dynamic_imports_from_function(&method.function, specifiers, seen);
            }
            ClassMember::PrivateProp(property) => {
                if let Some(value) = &property.value {
                    collect_dynamic_imports_from_expression(value, specifiers, seen);
                }
            }
            ClassMember::StaticBlock(block) => {
                for statement in &block.body.stmts {
                    collect_dynamic_imports_from_statement(statement, specifiers, seen);
                }
            }
            _ => {}
        }
    }
}

fn collect_dynamic_imports_from_property_name(
    name: &PropName,
    specifiers: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    if let PropName::Computed(computed) = name {
        collect_dynamic_imports_from_expression(&computed.expr, specifiers, seen);
    }
}

fn collect_dynamic_imports_from_expression(
    expression: &Expr,
    specifiers: &mut Vec<String>,
    seen: &mut HashSet<String>,
) {
    match expression {
        Expr::Call(call) => {
            if matches!(call.callee, Callee::Import(_))
                && call.args.len() == 1
                && call.args[0].spread.is_none()
                && let Expr::Lit(Lit::Str(string)) = &*call.args[0].expr
            {
                let source = string.value.to_string_lossy().to_string();
                if seen.insert(source.clone()) {
                    specifiers.push(source);
                }
            } else if let Callee::Expr(callee) = &call.callee {
                collect_dynamic_imports_from_expression(callee, specifiers, seen);
            }

            for argument in &call.args {
                collect_dynamic_imports_from_expression(&argument.expr, specifiers, seen);
            }
        }
        Expr::New(new_expression) => {
            collect_dynamic_imports_from_expression(&new_expression.callee, specifiers, seen);
            for argument in new_expression.args.iter().flatten() {
                collect_dynamic_imports_from_expression(&argument.expr, specifiers, seen);
            }
        }
        Expr::Await(await_expression) => {
            collect_dynamic_imports_from_expression(&await_expression.arg, specifiers, seen)
        }
        Expr::Yield(yield_expression) => {
            if let Some(argument) = &yield_expression.arg {
                collect_dynamic_imports_from_expression(argument, specifiers, seen);
            }
        }
        Expr::Paren(parenthesized) => {
            collect_dynamic_imports_from_expression(&parenthesized.expr, specifiers, seen)
        }
        Expr::Array(array) => {
            for element in array.elems.iter().flatten() {
                collect_dynamic_imports_from_expression(&element.expr, specifiers, seen);
            }
        }
        Expr::Object(object) => {
            for property in &object.props {
                match property {
                    PropOrSpread::Spread(spread) => {
                        collect_dynamic_imports_from_expression(&spread.expr, specifiers, seen)
                    }
                    PropOrSpread::Prop(property) => match &**property {
                        Prop::Shorthand(_) => {}
                        Prop::KeyValue(property) => {
                            collect_dynamic_imports_from_property_name(
                                &property.key,
                                specifiers,
                                seen,
                            );
                            collect_dynamic_imports_from_expression(
                                &property.value,
                                specifiers,
                                seen,
                            );
                        }
                        Prop::Getter(property) => {
                            collect_dynamic_imports_from_property_name(
                                &property.key,
                                specifiers,
                                seen,
                            );
                            if let Some(body) = &property.body {
                                for statement in &body.stmts {
                                    collect_dynamic_imports_from_statement(
                                        statement, specifiers, seen,
                                    );
                                }
                            }
                        }
                        Prop::Setter(property) => {
                            collect_dynamic_imports_from_property_name(
                                &property.key,
                                specifiers,
                                seen,
                            );
                            collect_dynamic_imports_from_pattern(&property.param, specifiers, seen);
                            if let Some(body) = &property.body {
                                for statement in &body.stmts {
                                    collect_dynamic_imports_from_statement(
                                        statement, specifiers, seen,
                                    );
                                }
                            }
                        }
                        Prop::Method(property) => {
                            collect_dynamic_imports_from_property_name(
                                &property.key,
                                specifiers,
                                seen,
                            );
                            collect_dynamic_imports_from_function(
                                &property.function,
                                specifiers,
                                seen,
                            );
                        }
                        Prop::Assign(property) => collect_dynamic_imports_from_expression(
                            &property.value,
                            specifiers,
                            seen,
                        ),
                    },
                }
            }
        }
        Expr::Member(member) => {
            collect_dynamic_imports_from_expression(&member.obj, specifiers, seen);
            if let MemberProp::Computed(property) = &member.prop {
                collect_dynamic_imports_from_expression(&property.expr, specifiers, seen);
            }
        }
        Expr::Unary(unary) => collect_dynamic_imports_from_expression(&unary.arg, specifiers, seen),
        Expr::Update(update) => {
            collect_dynamic_imports_from_expression(&update.arg, specifiers, seen)
        }
        Expr::Bin(binary) => {
            collect_dynamic_imports_from_expression(&binary.left, specifiers, seen);
            collect_dynamic_imports_from_expression(&binary.right, specifiers, seen);
        }
        Expr::Assign(assignment) => {
            match &assignment.left {
                AssignTarget::Simple(simple) => {
                    if let SimpleAssignTarget::Member(member) = simple {
                        collect_dynamic_imports_from_expression(&member.obj, specifiers, seen);
                        if let MemberProp::Computed(property) = &member.prop {
                            collect_dynamic_imports_from_expression(
                                &property.expr,
                                specifiers,
                                seen,
                            );
                        }
                    }
                }
                AssignTarget::Pat(_) => {}
            }
            collect_dynamic_imports_from_expression(&assignment.right, specifiers, seen);
        }
        Expr::Cond(conditional) => {
            collect_dynamic_imports_from_expression(&conditional.test, specifiers, seen);
            collect_dynamic_imports_from_expression(&conditional.cons, specifiers, seen);
            collect_dynamic_imports_from_expression(&conditional.alt, specifiers, seen);
        }
        Expr::Seq(sequence) => {
            for expression in &sequence.exprs {
                collect_dynamic_imports_from_expression(expression, specifiers, seen);
            }
        }
        Expr::Fn(function) => {
            collect_dynamic_imports_from_function(&function.function, specifiers, seen)
        }
        Expr::Arrow(arrow) => {
            for parameter in &arrow.params {
                collect_dynamic_imports_from_pattern(parameter, specifiers, seen);
            }
            match &*arrow.body {
                BlockStmtOrExpr::BlockStmt(block) => {
                    for statement in &block.stmts {
                        collect_dynamic_imports_from_statement(statement, specifiers, seen);
                    }
                }
                BlockStmtOrExpr::Expr(expression) => {
                    collect_dynamic_imports_from_expression(expression, specifiers, seen);
                }
            }
        }
        Expr::Class(class) => collect_dynamic_imports_from_class(&class.class, specifiers, seen),
        Expr::Tpl(template) => {
            for expression in &template.exprs {
                collect_dynamic_imports_from_expression(expression, specifiers, seen);
            }
        }
        Expr::TaggedTpl(tagged) => {
            collect_dynamic_imports_from_expression(&tagged.tag, specifiers, seen);
            for expression in &tagged.tpl.exprs {
                collect_dynamic_imports_from_expression(expression, specifiers, seen);
            }
        }
        _ => {}
    }
}

fn module_export_name_string(name: &ModuleExportName) -> Result<String> {
    match name {
        ModuleExportName::Ident(identifier) => Ok(identifier.sym.to_string()),
        ModuleExportName::Str(string) => string
            .value
            .as_str()
            .map(str::to_string)
            .context("malformed module export name"),
    }
}

fn define_property_statement(
    target: Expression,
    property: Expression,
    descriptor: Expression,
) -> Statement {
    Statement::Expression(Expression::Call {
        callee: Box::new(Expression::Member {
            object: Box::new(Expression::Identifier("Object".to_string())),
            property: Box::new(Expression::String("defineProperty".to_string())),
        }),
        arguments: vec![
            CallArgument::Expression(target),
            CallArgument::Expression(property),
            CallArgument::Expression(descriptor),
        ],
    })
}

fn data_property_descriptor(
    value: Expression,
    writable: bool,
    enumerable: bool,
    configurable: bool,
) -> Expression {
    Expression::Object(vec![
        ObjectEntry::Data {
            key: Expression::String("value".to_string()),
            value,
        },
        ObjectEntry::Data {
            key: Expression::String("writable".to_string()),
            value: Expression::Bool(writable),
        },
        ObjectEntry::Data {
            key: Expression::String("enumerable".to_string()),
            value: Expression::Bool(enumerable),
        },
        ObjectEntry::Data {
            key: Expression::String("configurable".to_string()),
            value: Expression::Bool(configurable),
        },
    ])
}

fn getter_property_descriptor(
    getter: Expression,
    enumerable: bool,
    configurable: bool,
) -> Expression {
    Expression::Object(vec![
        ObjectEntry::Data {
            key: Expression::String("get".to_string()),
            value: getter,
        },
        ObjectEntry::Data {
            key: Expression::String("enumerable".to_string()),
            value: Expression::Bool(enumerable),
        },
        ObjectEntry::Data {
            key: Expression::String("configurable".to_string()),
            value: Expression::Bool(configurable),
        },
    ])
}

fn setter_property_descriptor(
    setter: Expression,
    enumerable: bool,
    configurable: bool,
) -> Expression {
    Expression::Object(vec![
        ObjectEntry::Data {
            key: Expression::String("set".to_string()),
            value: setter,
        },
        ObjectEntry::Data {
            key: Expression::String("enumerable".to_string()),
            value: Expression::Bool(enumerable),
        },
        ObjectEntry::Data {
            key: Expression::String("configurable".to_string()),
            value: Expression::Bool(configurable),
        },
    ])
}

struct ImportBindingRewriter<'a> {
    import_bindings: &'a HashMap<String, ImportBinding>,
    scopes: Vec<HashSet<String>>,
}

#[derive(Default)]
struct BindingScope {
    names: Vec<String>,
    renames: HashMap<String, String>,
}

impl<'a> ImportBindingRewriter<'a> {
    fn new(import_bindings: &'a HashMap<String, ImportBinding>) -> Self {
        Self {
            import_bindings,
            scopes: Vec::new(),
        }
    }

    fn rewrite_statement_list(&mut self, statements: &mut [Statement]) -> Result<()> {
        self.scopes.push(
            collect_statement_bindings(statements.iter())
                .into_iter()
                .collect(),
        );
        for statement in statements {
            self.rewrite_statement(statement)?;
        }
        self.scopes.pop();
        Ok(())
    }

    fn rewrite_statement(&mut self, statement: &mut Statement) -> Result<()> {
        match statement {
            Statement::Block { body } | Statement::Labeled { body, .. } => {
                self.rewrite_statement_list(body)
            }
            Statement::Var { value, .. }
            | Statement::Let { value, .. }
            | Statement::Expression(value)
            | Statement::Throw(value)
            | Statement::Return(value)
            | Statement::Yield { value }
            | Statement::YieldDelegate { value } => self.rewrite_expression(value),
            Statement::Assign { name, value } => {
                if !self.is_shadowed(name)
                    && let Some(binding) = self.import_bindings.get(name)
                {
                    return match binding {
                        ImportBinding::Named {
                            namespace_param,
                            export_name,
                            ..
                        } => {
                            self.rewrite_expression(value)?;
                            let value = value.clone();
                            *statement = Statement::AssignMember {
                                object: Expression::Identifier(namespace_param.clone()),
                                property: Expression::String(export_name.clone()),
                                value,
                            };
                            Ok(())
                        }
                        ImportBinding::Namespace { .. } => {
                            bail!("assignment to namespace import `{name}` is not supported yet")
                        }
                    };
                }
                self.rewrite_expression(value)
            }
            Statement::AssignMember {
                object,
                property,
                value,
            } => {
                self.rewrite_expression(object)?;
                self.rewrite_expression(property)?;
                self.rewrite_expression(value)
            }
            Statement::Print { values } => {
                for value in values {
                    self.rewrite_expression(value)?;
                }
                Ok(())
            }
            Statement::With { object, body } => {
                self.rewrite_expression(object)?;
                self.rewrite_statement_list(body)
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.rewrite_expression(condition)?;
                self.rewrite_statement_list(then_branch)?;
                self.rewrite_statement_list(else_branch)
            }
            Statement::Try {
                body,
                catch_binding,
                catch_setup,
                catch_body,
            } => {
                self.rewrite_statement_list(body)?;
                let mut catch_scope: HashSet<String> =
                    collect_statement_bindings(catch_setup.iter().chain(catch_body.iter()))
                        .into_iter()
                        .collect();
                if let Some(catch_binding) = catch_binding {
                    catch_scope.insert(catch_binding.clone());
                }
                self.scopes.push(catch_scope);
                for statement in catch_setup {
                    self.rewrite_statement(statement)?;
                }
                for statement in catch_body {
                    self.rewrite_statement(statement)?;
                }
                self.scopes.pop();
                Ok(())
            }
            Statement::Switch {
                bindings,
                discriminant,
                cases,
                ..
            } => {
                self.rewrite_expression(discriminant)?;
                self.scopes.push(bindings.iter().cloned().collect());
                for case in cases {
                    if let Some(test) = &mut case.test {
                        self.rewrite_expression(test)?;
                    }
                    self.rewrite_statement_list(&mut case.body)?;
                }
                self.scopes.pop();
                Ok(())
            }
            Statement::For {
                init,
                per_iteration_bindings,
                condition,
                update,
                break_hook,
                body,
                ..
            } => {
                let mut loop_bindings: HashSet<String> = collect_statement_bindings(init.iter())
                    .into_iter()
                    .collect();
                loop_bindings.extend(per_iteration_bindings.iter().cloned());
                self.scopes.push(loop_bindings);
                for statement in init {
                    self.rewrite_statement(statement)?;
                }
                if let Some(condition) = condition {
                    self.rewrite_expression(condition)?;
                }
                if let Some(update) = update {
                    self.rewrite_expression(update)?;
                }
                if let Some(break_hook) = break_hook {
                    self.rewrite_expression(break_hook)?;
                }
                self.rewrite_statement_list(body)?;
                self.scopes.pop();
                Ok(())
            }
            Statement::While {
                condition,
                break_hook,
                body,
                ..
            }
            | Statement::DoWhile {
                condition,
                break_hook,
                body,
                ..
            } => {
                self.rewrite_expression(condition)?;
                if let Some(break_hook) = break_hook {
                    self.rewrite_expression(break_hook)?;
                }
                self.rewrite_statement_list(body)
            }
            Statement::Break { .. } | Statement::Continue { .. } => Ok(()),
        }
    }

    fn rewrite_expression(&mut self, expression: &mut Expression) -> Result<()> {
        match expression {
            Expression::Array(elements) => {
                for element in elements {
                    match element {
                        ArrayElement::Expression(expression) | ArrayElement::Spread(expression) => {
                            self.rewrite_expression(expression)?
                        }
                    }
                }
                Ok(())
            }
            Expression::Sequence(elements) => {
                for element in elements {
                    self.rewrite_expression(element)?;
                }
                Ok(())
            }
            Expression::Object(entries) => {
                for entry in entries {
                    match entry {
                        ObjectEntry::Data { key, value } => {
                            self.rewrite_expression(key)?;
                            self.rewrite_expression(value)?;
                        }
                        ObjectEntry::Getter { key, getter }
                        | ObjectEntry::Setter {
                            key,
                            setter: getter,
                        } => {
                            self.rewrite_expression(key)?;
                            self.rewrite_expression(getter)?;
                        }
                        ObjectEntry::Spread(expression) => {
                            self.rewrite_expression(expression)?;
                        }
                    }
                }
                Ok(())
            }
            Expression::Identifier(name) => {
                if !self.is_shadowed(name)
                    && let Some(binding) = self.import_bindings.get(name)
                {
                    *expression = import_binding_expression(binding);
                }
                Ok(())
            }
            Expression::Member { object, property } => {
                self.rewrite_expression(object)?;
                self.rewrite_expression(property)
            }
            Expression::SuperMember { property } => self.rewrite_expression(property),
            Expression::Assign { name, value } => {
                if !self.is_shadowed(name)
                    && let Some(binding) = self.import_bindings.get(name)
                {
                    return match binding {
                        ImportBinding::Named {
                            namespace_param,
                            export_name,
                            ..
                        } => {
                            self.rewrite_expression(value)?;
                            let value = value.as_ref().clone();
                            *expression = Expression::AssignMember {
                                object: Box::new(Expression::Identifier(namespace_param.clone())),
                                property: Box::new(Expression::String(export_name.clone())),
                                value: Box::new(value),
                            };
                            Ok(())
                        }
                        ImportBinding::Namespace { .. } => {
                            bail!("assignment to namespace import `{name}` is not supported yet")
                        }
                    };
                }
                self.rewrite_expression(value)
            }
            Expression::AssignMember {
                object,
                property,
                value,
            } => {
                self.rewrite_expression(object)?;
                self.rewrite_expression(property)?;
                self.rewrite_expression(value)
            }
            Expression::AssignSuperMember { property, value } => {
                self.rewrite_expression(property)?;
                self.rewrite_expression(value)
            }
            Expression::EnumerateKeys(expression)
            | Expression::Await(expression)
            | Expression::GetIterator(expression)
            | Expression::IteratorClose(expression)
            | Expression::Unary { expression, .. } => self.rewrite_expression(expression),
            Expression::Binary { left, right, .. } => {
                self.rewrite_expression(left)?;
                self.rewrite_expression(right)
            }
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                self.rewrite_expression(condition)?;
                self.rewrite_expression(then_expression)?;
                self.rewrite_expression(else_expression)
            }
            Expression::Call { callee, arguments }
            | Expression::SuperCall { callee, arguments }
            | Expression::New { callee, arguments } => {
                self.rewrite_expression(callee)?;
                for argument in arguments {
                    match argument {
                        CallArgument::Expression(argument) | CallArgument::Spread(argument) => {
                            self.rewrite_expression(argument)?
                        }
                    }
                }
                Ok(())
            }
            Expression::Update { name, .. } => {
                ensure!(
                    self.is_shadowed(name) || !self.import_bindings.contains_key(name),
                    "update of imported binding `{name}` is not supported yet"
                );
                Ok(())
            }
            Expression::Number(_)
            | Expression::BigInt(_)
            | Expression::String(_)
            | Expression::Bool(_)
            | Expression::Null
            | Expression::Undefined
            | Expression::NewTarget
            | Expression::This
            | Expression::Sent => Ok(()),
        }
    }

    fn rewrite_function(&mut self, function: &mut FunctionDeclaration) -> Result<()> {
        let mut function_scope: HashSet<String> = function
            .params
            .iter()
            .map(|parameter| parameter.name.clone())
            .collect();
        if let Some(self_binding) = &function.self_binding {
            function_scope.insert(self_binding.clone());
        }
        function_scope.insert("arguments".to_string());
        self.scopes.push(function_scope);
        self.rewrite_statement_list(&mut function.body)?;
        self.scopes.pop();
        Ok(())
    }

    fn is_shadowed(&self, name: &str) -> bool {
        self.scopes.iter().rev().any(|scope| scope.contains(name))
    }
}

fn rewrite_import_bindings_in_function(
    function: &mut FunctionDeclaration,
    import_bindings: &HashMap<String, ImportBinding>,
) -> Result<()> {
    ImportBindingRewriter::new(import_bindings).rewrite_function(function)
}

fn import_binding_expression(binding: &ImportBinding) -> Expression {
    match binding {
        ImportBinding::Namespace {
            namespace_param, ..
        } => Expression::Identifier(namespace_param.clone()),
        ImportBinding::Named {
            namespace_param,
            export_name,
            ..
        } => Expression::Member {
            object: Box::new(Expression::Identifier(namespace_param.clone())),
            property: Box::new(Expression::String(export_name.clone())),
        },
    }
}

fn collect_statement_bindings<'a>(statements: impl Iterator<Item = &'a Statement>) -> Vec<String> {
    let mut bindings = Vec::new();
    let mut seen = HashSet::new();
    for statement in statements {
        match statement {
            Statement::Var { name, .. } | Statement::Let { name, .. } => {
                if seen.insert(name.clone()) {
                    bindings.push(name.clone());
                }
            }
            _ => {}
        }
    }
    bindings
}

fn rewrite_script_await_identifiers(source: &str) -> Option<String> {
    #[derive(Clone, Copy)]
    enum State {
        Code,
        SingleQuoted,
        DoubleQuoted,
        Template,
        LineComment,
        BlockComment,
    }

    fn is_ident_start(character: char) -> bool {
        character == '_' || character == '$' || character.is_ascii_alphabetic()
    }

    fn is_ident_continue(character: char) -> bool {
        is_ident_start(character) || character.is_ascii_digit()
    }

    fn starts_unicode_escape(characters: &[char], index: usize) -> bool {
        matches!(
            characters.get(index..index + 6),
            Some(['\\', 'u', a, b, c, d])
                if a.is_ascii_hexdigit()
                    && b.is_ascii_hexdigit()
                    && c.is_ascii_hexdigit()
                    && d.is_ascii_hexdigit()
        )
    }

    fn decode_unicode_escape(characters: &[char], index: usize) -> Option<(usize, char)> {
        let digits = characters.get(index + 2..index + 6)?;
        let hex = digits.iter().collect::<String>();
        let value = u32::from_str_radix(&hex, 16).ok()?;
        Some((index + 6, char::from_u32(value)?))
    }

    let characters = source.chars().collect::<Vec<_>>();
    let mut rewritten = String::with_capacity(source.len());
    let mut state = State::Code;
    let mut index = 0;
    let mut changed = false;

    while index < characters.len() {
        let character = characters[index];
        let next = characters.get(index + 1).copied();

        match state {
            State::Code => {
                if character == '\'' {
                    state = State::SingleQuoted;
                    rewritten.push(character);
                    index += 1;
                    continue;
                }
                if character == '"' {
                    state = State::DoubleQuoted;
                    rewritten.push(character);
                    index += 1;
                    continue;
                }
                if character == '`' {
                    state = State::Template;
                    rewritten.push(character);
                    index += 1;
                    continue;
                }
                if character == '/' && next == Some('/') {
                    state = State::LineComment;
                    rewritten.push('/');
                    rewritten.push('/');
                    index += 2;
                    continue;
                }
                if character == '/' && next == Some('*') {
                    state = State::BlockComment;
                    rewritten.push('/');
                    rewritten.push('*');
                    index += 2;
                    continue;
                }
                if is_ident_start(character) || starts_unicode_escape(&characters, index) {
                    let mut word = String::new();
                    while index < characters.len() {
                        if is_ident_continue(characters[index]) {
                            word.push(characters[index]);
                            index += 1;
                        } else if starts_unicode_escape(&characters, index) {
                            let Some((next_index, decoded)) =
                                decode_unicode_escape(&characters, index)
                            else {
                                break;
                            };
                            word.push(decoded);
                            index = next_index;
                        } else {
                            break;
                        }
                    }

                    if word == "await" {
                        rewritten.push_str("__ayy_await_ident");
                        changed = true;
                    } else {
                        rewritten.push_str(&word);
                    }
                    continue;
                }

                rewritten.push(character);
                index += 1;
            }
            State::SingleQuoted => {
                rewritten.push(character);
                index += 1;
                if character == '\\' && index < characters.len() {
                    rewritten.push(characters[index]);
                    index += 1;
                } else if character == '\'' {
                    state = State::Code;
                }
            }
            State::DoubleQuoted => {
                rewritten.push(character);
                index += 1;
                if character == '\\' && index < characters.len() {
                    rewritten.push(characters[index]);
                    index += 1;
                } else if character == '"' {
                    state = State::Code;
                }
            }
            State::Template => {
                rewritten.push(character);
                index += 1;
                if character == '\\' && index < characters.len() {
                    rewritten.push(characters[index]);
                    index += 1;
                } else if character == '`' {
                    state = State::Code;
                }
            }
            State::LineComment => {
                rewritten.push(character);
                index += 1;
                if character == '\n' || character == '\r' {
                    state = State::Code;
                }
            }
            State::BlockComment => {
                rewritten.push(character);
                index += 1;
                if character == '*' && next == Some('/') {
                    rewritten.push('/');
                    index += 1;
                    state = State::Code;
                }
            }
        }
    }

    changed.then_some(rewritten)
}
