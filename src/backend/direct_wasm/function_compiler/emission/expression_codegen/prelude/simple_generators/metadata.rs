use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn is_async_generator_iterator_expression(
        &self,
        expression: &Expression,
    ) -> bool {
        if let Some(resolved) = self
            .resolve_bound_alias_expression(expression)
            .filter(|resolved| !static_expression_matches(resolved, expression))
        {
            return self.is_async_generator_iterator_expression(&resolved);
        }
        if let Expression::Identifier(name) = expression
            && let Some(value) = self
                .state
                .speculation
                .static_semantics
                .local_value_binding(name)
                .or_else(|| self.global_value_binding(name))
            && !static_expression_matches(value, expression)
        {
            return self.is_async_generator_iterator_expression(value);
        }
        let materialized = self.materialize_static_expression(expression);
        if !static_expression_matches(&materialized, expression) {
            return self.is_async_generator_iterator_expression(&materialized);
        }

        let Expression::Call { callee, .. } = expression else {
            return false;
        };
        let Some(LocalFunctionBinding::User(function_name)) =
            self.resolve_function_binding_from_expression(callee)
        else {
            return false;
        };
        self.user_function(&function_name)
            .is_some_and(|function| matches!(function.kind, FunctionKind::AsyncGenerator))
    }

    pub(in crate::backend::direct_wasm) fn simple_generator_source_metadata(
        &self,
        object: &Expression,
    ) -> Option<(bool, Vec<SimpleGeneratorStep>, Vec<Statement>, Expression)> {
        if let Expression::Identifier(name) = object
            && let Some(binding_name) = self.resolve_local_array_iterator_binding_name(name)
            && let Some(ArrayIteratorBinding {
                source:
                    IteratorSourceKind::SimpleGenerator {
                        is_async,
                        steps,
                        completion_effects,
                        completion_value,
                    },
                ..
            }) = self
                .state
                .speculation
                .static_semantics
                .local_array_iterator_binding(&binding_name)
        {
            return Some((
                *is_async,
                steps.clone(),
                completion_effects.clone(),
                completion_value.clone(),
            ));
        }
        if let Expression::Call { callee, .. } = object
            && let Some(LocalFunctionBinding::User(function_name)) =
                self.resolve_function_binding_from_expression(callee)
            && let Some(user_function) = self.user_function(&function_name)
        {
            let (steps, completion_effects, completion_value) =
                self.resolve_simple_generator_source(object)?;
            return Some((
                matches!(user_function.kind, FunctionKind::AsyncGenerator),
                steps,
                completion_effects,
                completion_value,
            ));
        }
        let materialized = self.materialize_static_expression(object);
        if !static_expression_matches(&materialized, object) {
            return self.simple_generator_source_metadata(&materialized);
        }

        let Expression::Call { callee, .. } = object else {
            return None;
        };
        let LocalFunctionBinding::User(function_name) =
            self.resolve_function_binding_from_expression(callee)?
        else {
            return None;
        };
        let user_function = self.user_function(&function_name)?;
        let (steps, completion_effects, completion_value) =
            self.resolve_simple_generator_source(object)?;
        Some((
            matches!(user_function.kind, FunctionKind::AsyncGenerator),
            steps,
            completion_effects,
            completion_value,
        ))
    }
}
