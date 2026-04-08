use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn expression_is_builtin_array_constructor(
        &self,
        expression: &Expression,
    ) -> bool {
        matches!(
            self.materialize_static_expression(expression),
            Expression::Identifier(name) if name == "Array"
        )
    }

    pub(in crate::backend::direct_wasm) fn expression_is_known_array_value(
        &self,
        expression: &Expression,
    ) -> bool {
        if self
            .resolve_array_binding_from_expression(expression)
            .is_some()
        {
            return true;
        }

        let materialized = self.materialize_static_expression(expression);
        if !static_expression_matches(&materialized, expression)
            && self
                .resolve_array_binding_from_expression(&materialized)
                .is_some()
        {
            return true;
        }

        self.resolve_bound_alias_expression(expression)
            .filter(|resolved| !static_expression_matches(resolved, expression))
            .is_some_and(|resolved| self.expression_is_known_array_value(&resolved))
    }

    pub(in crate::backend::direct_wasm) fn expression_is_known_non_object_value_for_instanceof(
        &self,
        expression: &Expression,
    ) -> bool {
        if self.expression_is_known_array_value(expression)
            || self.expression_is_known_function_value_for_instanceof(expression)
            || self.expression_is_known_promise_instance_for_instanceof(expression)
            || self.expression_is_known_constructor_instance_for_instanceof(expression, "WeakRef")
            || self.expression_is_known_native_error_instance_for_instanceof(expression, "Error")
        {
            return false;
        }
        if let Some(resolved) = self
            .resolve_bound_alias_expression(expression)
            .filter(|resolved| !static_expression_matches(resolved, expression))
        {
            return self.expression_is_known_non_object_value_for_instanceof(&resolved);
        }
        let materialized = self.materialize_static_expression(expression);
        if !static_expression_matches(&materialized, expression) {
            return self.expression_is_known_non_object_value_for_instanceof(&materialized);
        }
        matches!(
            self.infer_value_kind(&materialized),
            Some(
                StaticValueKind::Number
                    | StaticValueKind::Bool
                    | StaticValueKind::String
                    | StaticValueKind::BigInt
                    | StaticValueKind::Symbol
                    | StaticValueKind::Null
                    | StaticValueKind::Undefined
            )
        )
    }

    pub(in crate::backend::direct_wasm) fn expression_is_known_function_value_for_instanceof(
        &self,
        expression: &Expression,
    ) -> bool {
        if self
            .resolve_function_binding_from_expression(expression)
            .is_some()
        {
            return true;
        }
        if let Some(resolved) = self
            .resolve_bound_alias_expression(expression)
            .filter(|resolved| !static_expression_matches(resolved, expression))
        {
            return self.expression_is_known_function_value_for_instanceof(&resolved);
        }
        if matches!(
            expression,
            Expression::Call { callee, .. }
                if matches!(callee.as_ref(), Expression::Identifier(name)
                    if is_function_constructor_builtin(name))
        ) {
            return true;
        }
        let materialized = self.materialize_static_expression(expression);
        if !static_expression_matches(&materialized, expression) {
            return self.expression_is_known_function_value_for_instanceof(&materialized);
        }
        matches!(
            self.infer_value_kind(&materialized),
            Some(StaticValueKind::Function)
        ) || matches!(
            materialized,
            Expression::Call { ref callee, .. }
                if matches!(callee.as_ref(), Expression::Identifier(name)
                    if is_function_constructor_builtin(name))
        )
    }

    pub(in crate::backend::direct_wasm) fn expression_is_known_promise_instance_for_instanceof(
        &self,
        expression: &Expression,
    ) -> bool {
        if let Some(resolved) = self
            .resolve_bound_alias_expression(expression)
            .filter(|resolved| !static_expression_matches(resolved, expression))
        {
            return self.expression_is_known_promise_instance_for_instanceof(&resolved);
        }
        match expression {
            Expression::New { callee, .. } => {
                return matches!(callee.as_ref(), Expression::Identifier(name) if name == "Promise");
            }
            Expression::Call { callee, .. } => {
                if matches!(callee.as_ref(), Expression::Identifier(name) if name == "Promise") {
                    return true;
                }
                if matches!(
                    callee.as_ref(),
                    Expression::Member { object, property }
                        if matches!(object.as_ref(), Expression::Identifier(name) if name == "Promise")
                            && matches!(property.as_ref(), Expression::String(name) if name == "resolve")
                ) {
                    return true;
                }
                if self
                    .resolve_user_function_from_expression(callee.as_ref())
                    .is_some_and(|user_function| user_function.is_async())
                {
                    return true;
                }
            }
            _ => {}
        }
        let materialized = self.materialize_static_expression(expression);
        if !static_expression_matches(&materialized, expression) {
            return self.expression_is_known_promise_instance_for_instanceof(&materialized);
        }
        match materialized {
            Expression::New { callee, .. } => {
                matches!(callee.as_ref(), Expression::Identifier(name) if name == "Promise")
            }
            Expression::Call { callee, .. } => {
                if matches!(callee.as_ref(), Expression::Identifier(name) if name == "Promise") {
                    return true;
                }
                if matches!(
                    callee.as_ref(),
                    Expression::Member { object, property }
                        if matches!(object.as_ref(), Expression::Identifier(name) if name == "Promise")
                            && matches!(property.as_ref(), Expression::String(name) if name == "resolve")
                ) {
                    return true;
                }
                self.resolve_user_function_from_expression(callee.as_ref())
                    .is_some_and(|user_function| user_function.is_async())
            }
            _ => false,
        }
    }

    pub(in crate::backend::direct_wasm) fn expression_is_known_constructor_instance_for_instanceof(
        &self,
        expression: &Expression,
        constructor_name: &str,
    ) -> bool {
        if let Some(resolved) = self
            .resolve_bound_alias_expression(expression)
            .filter(|resolved| !static_expression_matches(resolved, expression))
        {
            return self.expression_is_known_constructor_instance_for_instanceof(
                &resolved,
                constructor_name,
            );
        }
        match expression {
            Expression::New { callee, .. } => {
                return matches!(callee.as_ref(), Expression::Identifier(name) if name == constructor_name);
            }
            Expression::Call { callee, .. } => {
                return matches!(callee.as_ref(), Expression::Identifier(name) if name == constructor_name)
                    && (constructor_name == "AggregateError"
                        || native_error_runtime_value(constructor_name).is_some());
            }
            _ => {}
        }
        let materialized = self.materialize_static_expression(expression);
        if !static_expression_matches(&materialized, expression) {
            return self.expression_is_known_constructor_instance_for_instanceof(
                &materialized,
                constructor_name,
            );
        }
        match materialized {
            Expression::New { callee, .. } => {
                matches!(callee.as_ref(), Expression::Identifier(name) if name == constructor_name)
            }
            Expression::Call { callee, .. } => {
                matches!(callee.as_ref(), Expression::Identifier(name) if name == constructor_name)
                    && (constructor_name == "AggregateError"
                        || native_error_runtime_value(constructor_name).is_some())
            }
            _ => false,
        }
    }

    pub(in crate::backend::direct_wasm) fn expression_is_known_native_error_instance_for_instanceof(
        &self,
        expression: &Expression,
        constructor_name: &str,
    ) -> bool {
        if constructor_name == "Error" {
            return NATIVE_ERROR_NAMES.iter().any(|candidate| {
                self.expression_is_known_constructor_instance_for_instanceof(expression, candidate)
            });
        }
        self.expression_is_known_constructor_instance_for_instanceof(expression, constructor_name)
    }

    pub(in crate::backend::direct_wasm) fn expression_is_known_object_like_value_for_instanceof(
        &self,
        expression: &Expression,
    ) -> bool {
        if self.expression_is_known_array_value(expression)
            || self.expression_is_known_function_value_for_instanceof(expression)
            || self.expression_is_known_promise_instance_for_instanceof(expression)
            || self.expression_is_known_constructor_instance_for_instanceof(expression, "WeakRef")
            || self.expression_is_known_native_error_instance_for_instanceof(expression, "Error")
        {
            return true;
        }
        if let Some(resolved) = self
            .resolve_bound_alias_expression(expression)
            .filter(|resolved| !static_expression_matches(resolved, expression))
        {
            return self.expression_is_known_object_like_value_for_instanceof(&resolved);
        }
        let materialized = self.materialize_static_expression(expression);
        if !static_expression_matches(&materialized, expression) {
            return self.expression_is_known_object_like_value_for_instanceof(&materialized);
        }
        matches!(
            self.infer_value_kind(&materialized),
            Some(StaticValueKind::Object)
        )
    }
}
