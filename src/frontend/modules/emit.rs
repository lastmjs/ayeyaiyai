use super::*;

impl ModuleLinker {
    pub(super) fn lower_default_export_declaration(
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

    pub(super) fn build_module_namespace_prelude(&self, exports_param: &str) -> Vec<Statement> {
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

    pub(super) fn build_export_getter_statements(
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

    pub(super) fn module_registry_statements(&self) -> Vec<Statement> {
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

    pub(super) fn module_init_call_arguments(&self, module_index: usize) -> Vec<CallArgument> {
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

    pub(super) fn bundle_statements(&self, entry_index: usize) -> Result<Vec<Statement>> {
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

    pub(super) fn rewrite_import_bindings_in_statements(
        &self,
        statements: &mut [Statement],
        import_bindings: &HashMap<String, ImportBinding>,
    ) -> Result<()> {
        let mut rewriter = import_rewriter::ImportBindingRewriter::new(import_bindings);
        rewriter.rewrite_statement_list(statements)
    }
}
