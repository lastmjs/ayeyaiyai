use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_primitive_property_key_expression(
        &self,
        expression: &Expression,
    ) -> Option<Expression> {
        if let Some(resolved) = self
            .resolve_bound_alias_expression(expression)
            .filter(|resolved| !static_expression_matches(resolved, expression))
        {
            if self.well_known_symbol_name(&resolved).is_some() {
                return Some(resolved);
            }
            if let Some(symbol_identity) = self.resolve_symbol_identity_expression(&resolved) {
                return Some(symbol_identity);
            }
        }
        if let Expression::Call { callee, arguments } = expression
            && matches!(callee.as_ref(), Expression::Identifier(name) if name == "String" && self.is_unshadowed_builtin_identifier(name))
        {
            let text = match arguments.first() {
                Some(CallArgument::Expression(argument)) | Some(CallArgument::Spread(argument)) => {
                    let materialized_argument = self.materialize_static_expression(argument);
                    let current_function_name = self.current_function_name();
                    if let Some(primitive) = self
                        .resolve_static_primitive_expression_with_context(
                            &materialized_argument,
                            current_function_name,
                        )
                        .or_else(|| {
                            self.resolve_static_primitive_expression_with_context(
                                argument,
                                current_function_name,
                            )
                        })
                        && let Some(property_name) =
                            static_property_name_from_expression(&primitive)
                    {
                        property_name
                    } else if let Some(binding) = self
                        .resolve_function_binding_from_expression_with_context(
                            &materialized_argument,
                            current_function_name,
                        )
                        .or_else(|| {
                            self.resolve_function_binding_from_expression_with_context(
                                argument,
                                current_function_name,
                            )
                        })
                    {
                        self.synthesize_static_function_binding_to_string(&binding)
                    } else {
                        self.resolve_static_string_value_with_context(
                            &materialized_argument,
                            current_function_name,
                        )
                        .or_else(|| {
                            self.resolve_static_string_value_with_context(
                                argument,
                                current_function_name,
                            )
                        })?
                    }
                }
                None => String::new(),
            };
            return Some(Expression::String(text));
        }
        let materialized = self.materialize_static_expression(expression);
        if let Some(binding) = self
            .resolve_function_binding_from_expression_with_context(
                &materialized,
                self.current_function_name(),
            )
            .or_else(|| {
                self.resolve_function_binding_from_expression_with_context(
                    expression,
                    self.current_function_name(),
                )
            })
        {
            return Some(Expression::String(
                self.synthesize_static_function_binding_to_string(&binding),
            ));
        }
        if let Some(text) = self
            .resolve_static_string_value_with_context(&materialized, self.current_function_name())
        {
            return Some(Expression::String(text));
        }
        if let Some(text) =
            self.resolve_static_string_value_with_context(expression, self.current_function_name())
        {
            return Some(Expression::String(text));
        }
        if let Some(primitive) = self.resolve_static_primitive_expression_with_context(
            &materialized,
            self.current_function_name(),
        ) {
            if let Some(property_name) = static_property_name_from_expression(&primitive) {
                return Some(Expression::String(property_name));
            }
        }
        if let Some(primitive) = self.resolve_static_primitive_expression_with_context(
            expression,
            self.current_function_name(),
        ) {
            if let Some(property_name) = static_property_name_from_expression(&primitive) {
                return Some(Expression::String(property_name));
            }
        }
        if let Some(property_name) = static_property_name_from_expression(&materialized) {
            return Some(Expression::String(property_name));
        }
        if self.well_known_symbol_name(&materialized).is_some() {
            return Some(materialized);
        }
        if self.well_known_symbol_name(expression).is_some() {
            return Some(expression.clone());
        }
        self.resolve_symbol_identity_expression(&materialized)
            .or_else(|| self.resolve_symbol_identity_expression(expression))
    }
}
