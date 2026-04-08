use super::*;

impl Lowerer {
    pub(crate) fn lower_variable_declaration(
        &mut self,
        variable_declaration: &swc_ecma_ast::VarDecl,
    ) -> Result<Vec<Statement>> {
        let mut lowered = Vec::new();

        for declarator in &variable_declaration.decls {
            if let Pat::Ident(identifier) = &declarator.name {
                let name = self.resolve_binding_name(identifier.id.sym.as_ref());

                if matches!(variable_declaration.kind, VarDeclKind::Var) {
                    let value = match declarator.init.as_deref() {
                        Some(initializer) => self.lower_expression_with_name_hint(
                            initializer,
                            Some(identifier.id.sym.as_ref()),
                        )?,
                        None => Expression::Undefined,
                    };

                    lowered.push(Statement::Var { name, value });
                } else {
                    let value = match declarator.init.as_deref() {
                        Some(initializer) => self.lower_expression_with_name_hint(
                            initializer,
                            Some(identifier.id.sym.as_ref()),
                        )?,
                        None => Expression::Undefined,
                    };

                    lowered.push(Statement::Let {
                        name,
                        mutable: !matches!(variable_declaration.kind, VarDeclKind::Const),
                        value,
                    });
                }

                continue;
            }

            if matches!(variable_declaration.kind, VarDeclKind::Var) {
                let mut names = Vec::new();
                collect_pattern_binding_names(&declarator.name, &mut names)?;
                for name in names {
                    lowered.push(Statement::Var {
                        name,
                        value: Expression::Undefined,
                    });
                }
            }

            let temporary_name = self.fresh_temporary_name("decl");
            let value = match declarator.init.as_deref() {
                Some(initializer) => self.lower_expression_with_name_hint(
                    initializer,
                    pattern_name_hint(&declarator.name),
                )?,
                None => Expression::Undefined,
            };
            lowered.push(Statement::Let {
                name: temporary_name.clone(),
                mutable: true,
                value,
            });
            self.lower_for_of_pattern_binding(
                &declarator.name,
                Expression::Identifier(temporary_name),
                if matches!(variable_declaration.kind, VarDeclKind::Var) {
                    ForOfPatternBindingKind::Assignment
                } else {
                    ForOfPatternBindingKind::Lexical {
                        mutable: !matches!(variable_declaration.kind, VarDeclKind::Const),
                    }
                },
                &mut lowered,
            )?;
        }

        Ok(lowered)
    }

    pub(crate) fn lower_nested_function_declaration(
        &mut self,
        function_declaration: &FnDecl,
    ) -> Result<Vec<Statement>> {
        self.next_function_expression_id += 1;
        let generated_name = format!(
            "__ayy_fnstmt_{}_{}",
            function_declaration.ident.sym, self.next_function_expression_id
        );
        let kind = lower_function_kind(
            function_declaration.function.is_generator,
            function_declaration.function.is_async,
        );
        let extra_bindings = vec![function_declaration.ident.sym.to_string()];
        let (params, body) =
            self.lower_function_parts(&function_declaration.function, &extra_bindings)?;

        self.functions.push(FunctionDeclaration {
            name: generated_name.clone(),
            top_level_binding: None,
            params,
            body,
            register_global: false,
            kind,
            self_binding: None,
            mapped_arguments: self.function_has_mapped_arguments(&function_declaration.function),
            strict: self.function_strict_mode(&function_declaration.function),
            lexical_this: false,
            derived_constructor: false,
            length: expected_argument_count(
                function_declaration
                    .function
                    .params
                    .iter()
                    .map(|parameter| &parameter.pat),
            ),
        });

        Ok(vec![Statement::Let {
            name: self.resolve_binding_name(function_declaration.ident.sym.as_ref()),
            mutable: true,
            value: Expression::Identifier(generated_name),
        }])
    }
}
