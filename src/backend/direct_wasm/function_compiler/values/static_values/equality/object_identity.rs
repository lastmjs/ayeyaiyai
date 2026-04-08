use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_static_reference_identity_key(
        &self,
        expression: &Expression,
    ) -> Option<String> {
        if matches!(expression, Expression::This) {
            return Some("this".to_string());
        }

        if let Some((resolved, callee_function_name)) = match expression {
            Expression::Call { callee, arguments } => self
                .resolve_static_call_result_expression_with_context(
                    callee,
                    arguments,
                    self.current_function_name(),
                ),
            _ => None,
        } && !static_expression_matches(&resolved, expression)
            && let Some(key) = self.resolve_static_reference_identity_key(&resolved)
        {
            let _ = callee_function_name;
            return Some(key);
        }

        if let Some(resolved) = self.resolve_bound_alias_expression(expression)
            && !static_expression_matches(&resolved, expression)
            && let Some(key) = self.resolve_static_reference_identity_key(&resolved)
        {
            return Some(key);
        }

        if let Expression::Identifier(name) = expression
            && let Some(key) = self.reference_identity_key_for_identifier(name)
        {
            return Some(key);
        }

        if let Some(function) = self.resolve_user_function_from_expression(expression) {
            return Some(format!("user-function:{}", function.name));
        }

        match expression {
            Expression::This => Some("this".to_string()),
            _ => self
                .resolve_user_function_from_expression(expression)
                .map(|function| format!("user-function:{}", function.name)),
        }
    }

    pub(in crate::backend::direct_wasm) fn reference_identity_key_for_identifier(
        &self,
        name: &str,
    ) -> Option<String> {
        if let Some((resolved_name, _)) = self.resolve_current_local_binding(name)
            && (self
                .state
                .speculation
                .static_semantics
                .has_local_array_binding(&resolved_name)
                || self
                    .state
                    .speculation
                    .static_semantics
                    .has_local_object_binding(&resolved_name)
                || self
                    .state
                    .speculation
                    .static_semantics
                    .has_local_function_binding(&resolved_name))
        {
            return Some(format!("local:{resolved_name}"));
        }
        if self
            .state
            .speculation
            .static_semantics
            .has_local_array_binding(name)
            || self
                .state
                .speculation
                .static_semantics
                .has_local_object_binding(name)
            || self
                .state
                .speculation
                .static_semantics
                .has_local_function_binding(name)
        {
            return Some(format!("local:{name}"));
        }
        if self.backend.global_array_binding(name).is_some()
            || self.backend.global_object_binding(name).is_some()
            || self.backend.global_function_binding(name).is_some()
        {
            return Some(format!("global:{name}"));
        }
        None
    }

    pub(in crate::backend::direct_wasm) fn resolve_static_object_identity_expression(
        &self,
        expression: &Expression,
    ) -> Option<Expression> {
        if self
            .resolve_static_object_prototype_expression(expression)
            .is_none()
        {
            return None;
        }
        if let Some(resolved) = self
            .resolve_bound_alias_expression(expression)
            .filter(|resolved| !static_expression_matches(resolved, expression))
        {
            return self.resolve_static_object_identity_expression(&resolved);
        }
        match expression {
            Expression::Array(_)
            | Expression::Object(_)
            | Expression::New { .. }
            | Expression::Member { .. }
            | Expression::This => Some(expression.clone()),
            Expression::Call { .. }
                if self
                    .resolve_static_weakref_target_expression(expression)
                    .is_some()
                    || self.expression_is_known_promise_instance_for_instanceof(expression) =>
            {
                Some(expression.clone())
            }
            Expression::Identifier(_) => Some(expression.clone()),
            _ => {
                let materialized = self.materialize_static_expression(expression);
                if !static_expression_matches(&materialized, expression) {
                    self.resolve_static_object_identity_expression(&materialized)
                } else {
                    None
                }
            }
        }
    }
}
