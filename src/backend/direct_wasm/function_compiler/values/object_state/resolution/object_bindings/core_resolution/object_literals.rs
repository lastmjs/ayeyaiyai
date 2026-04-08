use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn resolve_object_literal_expression_binding(
        &self,
        expression: &Expression,
    ) -> Option<ObjectValueBinding> {
        let Expression::Object(entries) = expression else {
            return None;
        };
        let mut environment = self.snapshot_static_resolution_environment_without_locals();
        self.resolve_object_binding_entries_with_state(entries, &mut environment)
    }
}
