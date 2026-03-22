use super::*;

pub(super) fn module_export_name_string(name: &ModuleExportName) -> Result<String> {
    match name {
        ModuleExportName::Ident(identifier) => Ok(identifier.sym.to_string()),
        ModuleExportName::Str(string) => string
            .value
            .as_str()
            .map(str::to_string)
            .context("malformed module export name"),
    }
}

impl ModuleLinker {
    pub(super) fn predeclare_module_export_resolutions(
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

    pub(super) fn require_export_resolution_for_dependency(
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

    pub(super) fn require_export_resolution_for_dependency_with_visited(
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

    pub(super) fn export_resolution_for_import_binding(
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

    pub(super) fn register_import_declaration(
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

    pub(super) fn dependency_param_for_source(
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
}
