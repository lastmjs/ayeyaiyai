use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_property_key_from_function_binding(
        &self,
        binding: &LocalFunctionBinding,
    ) -> Option<Expression> {
        if let LocalFunctionBinding::User(function_name) = binding
            && let Some(user_function) = self.user_function(function_name)
            && let Some(summary) = user_function.inline_summary.as_ref()
            && let Some(return_value) = summary.return_value.as_ref()
        {
            let substituted =
                self.substitute_user_function_argument_bindings(return_value, user_function, &[]);
            if let Some(key) = self.resolve_primitive_property_key_expression(&substituted) {
                return Some(key);
            }
        }

        match self.resolve_terminal_function_outcome_from_binding(binding, &[])? {
            StaticEvalOutcome::Value(expression) => {
                self.resolve_primitive_property_key_expression(&expression)
            }
            StaticEvalOutcome::Throw(_) => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_property_key_coercion_from_object_binding(
        &self,
        object_binding: &ObjectValueBinding,
    ) -> Option<(LocalFunctionBinding, Expression)> {
        for method_name in ["toString", "valueOf"] {
            let method_value = object_binding_lookup_value(
                object_binding,
                &Expression::String(method_name.to_string()),
            );
            match method_value {
                None | Some(Expression::Null) | Some(Expression::Undefined) => continue,
                Some(value) => {
                    let binding = self.resolve_function_binding_from_expression(value)?;
                    let key = self.resolve_property_key_from_function_binding(&binding)?;
                    return Some((binding, key));
                }
            }
        }
        None
    }

    pub(in crate::backend::direct_wasm) fn resolve_property_key_expression_with_coercion(
        &self,
        expression: &Expression,
    ) -> Option<ResolvedPropertyKey> {
        if let Some(key) = self.resolve_primitive_property_key_expression(expression) {
            return Some(ResolvedPropertyKey {
                key,
                coercion: None,
            });
        }

        let object_binding = match expression {
            Expression::Object(_) => None,
            _ => self.resolve_object_binding_from_expression(expression),
        }
        .or_else(|| {
            let materialized = self.materialize_static_expression(expression);
            match materialized {
                Expression::Object(_) => None,
                _ => self.resolve_object_binding_from_expression(&materialized),
            }
        })?;
        let (coercion, key) =
            self.resolve_property_key_coercion_from_object_binding(&object_binding)?;
        Some(ResolvedPropertyKey {
            key,
            coercion: Some(coercion),
        })
    }

    pub(in crate::backend::direct_wasm) fn resolve_property_key_expression(
        &self,
        expression: &Expression,
    ) -> Option<Expression> {
        self.resolve_property_key_expression_with_coercion(expression)
            .map(|resolved| resolved.key)
    }
}
