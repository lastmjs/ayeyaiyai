use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn resolve_iterator_expression_object_binding(
        &self,
        expression: &Expression,
    ) -> Option<ObjectValueBinding> {
        let Expression::GetIterator(iterated) = expression else {
            return None;
        };
        if let Some(object_binding) = self.resolve_object_binding_from_expression(iterated) {
            let has_next_method = object_binding_lookup_value(
                &object_binding,
                &Expression::String("next".to_string()),
            )
            .and_then(|value| self.resolve_function_binding_from_expression(value))
            .is_some();
            if has_next_method || self.resolve_iterator_source_kind(iterated).is_some() {
                return Some(object_binding);
            }
        }
        let iterator_callee = Expression::Member {
            object: Box::new((**iterated).clone()),
            property: Box::new(self.materialize_static_expression(&symbol_iterator_expression())),
        };
        self.resolve_object_binding_from_expression(&Expression::Call {
            callee: Box::new(iterator_callee),
            arguments: Vec::new(),
        })
    }
}
