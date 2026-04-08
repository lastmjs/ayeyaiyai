use super::*;

#[path = "await_resolution/bound_snapshot.rs"]
mod bound_snapshot;
#[path = "await_resolution/static_resolution.rs"]
mod static_resolution;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn static_expression_is_object_like(
        &self,
        expression: &Expression,
    ) -> bool {
        self.resolve_iterator_source_kind(expression).is_some()
            || self
                .resolve_object_binding_from_expression(expression)
                .is_some()
            || matches!(
                self.infer_value_kind(expression),
                Some(StaticValueKind::Object | StaticValueKind::Function)
            )
    }

    pub(in crate::backend::direct_wasm) fn resolve_static_property_get_outcome(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> Option<StaticEvalOutcome> {
        let property = self.materialize_static_expression(property);
        if let Some(getter_binding) = self.resolve_member_getter_binding(object, &property) {
            return self.resolve_static_function_outcome_from_binding_with_context(
                &getter_binding,
                &[],
                self.current_function_name(),
            );
        }
        let object_binding = self.resolve_object_binding_from_expression(object)?;
        Some(StaticEvalOutcome::Value(
            object_binding_lookup_value(&object_binding, &property)
                .cloned()
                .unwrap_or(Expression::Undefined),
        ))
    }
}
