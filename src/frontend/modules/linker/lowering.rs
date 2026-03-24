use super::super::*;

impl ModuleLinker {
    pub(crate) fn lower_module(
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
                            if export_resolutions.contains_key(export_name.as_str())
                                || ambiguous_star_exports.contains(export_name.as_str())
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
            kind: FunctionKind::from_flags(false, init_async),
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
}
