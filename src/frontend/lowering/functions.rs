use super::*;

impl Lowerer {
    pub(crate) fn lower_named_default_function_expression(
        &mut self,
        function_expression: &FnExpr,
    ) -> Result<String> {
        self.next_function_expression_id += 1;
        let identifier = function_expression
            .ident
            .as_ref()
            .context("named default export function must have an identifier")?;
        let generated_name = format!(
            "__ayy_fnstmt_{}_{}",
            identifier.sym, self.next_function_expression_id
        );
        let kind = lower_function_kind(
            function_expression.function.is_generator,
            function_expression.function.is_async,
        );
        let extra_bindings = vec![identifier.sym.to_string()];
        let (params, body) =
            self.lower_function_parts(&function_expression.function, &extra_bindings)?;

        self.functions.push(FunctionDeclaration {
            name: generated_name.clone(),
            top_level_binding: None,
            params,
            body,
            register_global: false,
            kind,
            self_binding: None,
            mapped_arguments: self.function_has_mapped_arguments(&function_expression.function),
            strict: self.function_strict_mode(&function_expression.function),
            lexical_this: false,
            derived_constructor: false,
            length: expected_argument_count(
                function_expression
                    .function
                    .params
                    .iter()
                    .map(|parameter| &parameter.pat),
            ),
        });

        Ok(generated_name)
    }

    pub(crate) fn lower_function_declaration(
        &mut self,
        function_declaration: &FnDecl,
        register_global: bool,
    ) -> Result<FunctionDeclaration> {
        ensure!(
            !function_declaration.declare,
            "declare function is not supported yet"
        );

        let extra_bindings = vec![function_declaration.ident.sym.to_string()];
        let (params, body) =
            self.lower_function_parts(&function_declaration.function, &extra_bindings)?;

        Ok(FunctionDeclaration {
            name: function_declaration.ident.sym.to_string(),
            top_level_binding: None,
            params,
            body,
            register_global,
            kind: lower_function_kind(
                function_declaration.function.is_generator,
                function_declaration.function.is_async,
            ),
            self_binding: Some(function_declaration.ident.sym.to_string()),
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
        })
    }

    pub(crate) fn lower_function_expression(
        &mut self,
        function_expression: &FnExpr,
        name_hint: Option<&str>,
    ) -> Result<Expression> {
        self.next_function_expression_id += 1;
        let generated_name = match &function_expression.ident {
            Some(identifier) => {
                format!(
                    "__ayy_fnexpr_{}_{}",
                    identifier.sym, self.next_function_expression_id
                )
            }
            None => match name_hint {
                Some(name_hint) => format!(
                    "__ayy_fnexpr_{}__name_{}",
                    self.next_function_expression_id, name_hint
                ),
                None => format!("__ayy_fnexpr_{}", self.next_function_expression_id),
            },
        };
        let kind = lower_function_kind(
            function_expression.function.is_generator,
            function_expression.function.is_async,
        );
        let extra_bindings = function_expression
            .ident
            .as_ref()
            .map(|identifier| vec![identifier.sym.to_string()])
            .unwrap_or_default();
        let (params, body) =
            self.lower_function_parts(&function_expression.function, &extra_bindings)?;
        let self_binding = function_expression
            .ident
            .as_ref()
            .map(|identifier| identifier.sym.to_string());

        self.functions.push(FunctionDeclaration {
            name: generated_name.clone(),
            top_level_binding: None,
            params,
            body,
            register_global: false,
            kind,
            self_binding,
            mapped_arguments: self.function_has_mapped_arguments(&function_expression.function),
            strict: self.function_strict_mode(&function_expression.function),
            lexical_this: false,
            derived_constructor: false,
            length: expected_argument_count(
                function_expression
                    .function
                    .params
                    .iter()
                    .map(|parameter| &parameter.pat),
            ),
        });

        Ok(Expression::Identifier(generated_name))
    }

    pub(crate) fn lower_arrow_expression(
        &mut self,
        arrow_expression: &ArrowExpr,
        name_hint: Option<&str>,
    ) -> Result<Expression> {
        self.next_function_expression_id += 1;
        let generated_name = match name_hint {
            Some(name_hint) => format!(
                "__ayy_arrow_{}__name_{}",
                self.next_function_expression_id, name_hint
            ),
            None => format!("__ayy_arrow_{}", self.next_function_expression_id),
        };

        let (params, param_setup) = lower_parameter_patterns(self, arrow_expression.params.iter())?;

        let mut body = match &*arrow_expression.body {
            BlockStmtOrExpr::BlockStmt(block) => {
                self.lower_statements(&block.stmts, true, false)?
            }
            BlockStmtOrExpr::Expr(expression) => vec![Statement::Return(
                self.lower_expression_with_name_hint(expression, None)?,
            )],
        };
        body.splice(0..0, param_setup);

        self.functions.push(FunctionDeclaration {
            name: generated_name.clone(),
            top_level_binding: None,
            params,
            body,
            register_global: false,
            kind: FunctionKind::from_flags(false, arrow_expression.is_async),
            self_binding: None,
            mapped_arguments: false,
            strict: self.arrow_strict_mode(arrow_expression),
            lexical_this: true,
            derived_constructor: false,
            length: expected_argument_count(arrow_expression.params.iter()),
        });

        Ok(Expression::Identifier(generated_name))
    }

    pub(crate) fn lower_function_parts(
        &mut self,
        function: &Function,
        extra_bindings: &[String],
    ) -> Result<(Vec<Parameter>, Vec<Statement>)> {
        let body = function
            .body
            .as_ref()
            .context("functions must have a body")?;
        let strict_mode = self.function_strict_mode(function);
        self.strict_modes.push(strict_mode);
        let mut scope_bindings = collect_parameter_binding_names(
            function.params.iter().map(|parameter| &parameter.pat),
        )?;
        for binding in collect_function_scope_binding_names(&body.stmts)? {
            if !scope_bindings.contains(&binding) {
                scope_bindings.push(binding);
            }
        }
        if !scope_bindings.iter().any(|binding| binding == "arguments") {
            scope_bindings.push("arguments".to_string());
        }
        for binding in extra_bindings {
            if !scope_bindings.contains(binding) {
                scope_bindings.push(binding.clone());
            }
        }

        self.push_binding_scope(scope_bindings);
        let lowered = (|| -> Result<(Vec<Parameter>, Vec<Statement>)> {
            let (params, param_setup) = lower_parameters(self, function)?;
            let mut body = if function.is_generator {
                self.lower_generator_statements(&body.stmts, true)?
            } else {
                self.lower_statements(&body.stmts, true, false)?
            };
            body.splice(0..0, param_setup);
            Ok((params, body))
        })();
        self.pop_binding_scope();
        self.strict_modes.pop();
        lowered.map(|(params, body)| {
            let body = if function.is_async && function.is_generator {
                asyncify_statements(body).0
            } else {
                body
            };
            (params, body)
        })
    }
}
