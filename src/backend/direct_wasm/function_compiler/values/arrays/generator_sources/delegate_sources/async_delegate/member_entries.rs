use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn expression_has_async_iterator_entry(
        &self,
        expression: &Expression,
    ) -> bool {
        let async_iterator_property = self.materialize_static_expression(&Expression::Member {
            object: Box::new(Expression::Identifier("Symbol".to_string())),
            property: Box::new(Expression::String("asyncIterator".to_string())),
        });
        let current_function_name = self.current_function_name();
        if let Some(getter_binding) =
            self.resolve_member_getter_binding(expression, &async_iterator_property)
        {
            return match self.resolve_static_function_outcome_from_binding_with_context(
                &getter_binding,
                &[],
                current_function_name,
            ) {
                Some(StaticEvalOutcome::Value(method_value)) => {
                    !matches!(method_value, Expression::Undefined | Expression::Null)
                }
                Some(StaticEvalOutcome::Throw(_)) => false,
                None => true,
            };
        }
        if self
            .resolve_member_function_binding(expression, &async_iterator_property)
            .is_some()
        {
            return true;
        }
        self.resolve_object_binding_from_expression(expression)
            .and_then(|object_binding| {
                object_binding_lookup_value(&object_binding, &async_iterator_property).cloned()
            })
            .is_some_and(|method_value| {
                !matches!(method_value, Expression::Undefined | Expression::Null)
            })
    }

    pub(in crate::backend::direct_wasm) fn expression_has_defined_member_entry(
        &self,
        expression: &Expression,
        property: &Expression,
    ) -> bool {
        let current_function_name = self.current_function_name();
        if let Some(getter_binding) = self.resolve_member_getter_binding(expression, property) {
            return match self.resolve_static_function_outcome_from_binding_with_context(
                &getter_binding,
                &[],
                current_function_name,
            ) {
                Some(StaticEvalOutcome::Value(value)) => {
                    !matches!(value, Expression::Undefined | Expression::Null)
                }
                Some(StaticEvalOutcome::Throw(_)) => true,
                None => true,
            };
        }
        if self
            .resolve_member_function_binding(expression, property)
            .is_some()
        {
            return true;
        }
        self.resolve_object_binding_from_expression(expression)
            .and_then(|object_binding| {
                object_binding_lookup_value(&object_binding, property).cloned()
            })
            .is_some_and(|value| !matches!(value, Expression::Undefined | Expression::Null))
    }
}
