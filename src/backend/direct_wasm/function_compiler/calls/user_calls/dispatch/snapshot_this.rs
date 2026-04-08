use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_static_snapshot_this_expression(
        &self,
        this_expression: &Expression,
    ) -> Expression {
        if !matches!(this_expression, Expression::This) {
            return self.materialize_static_expression(this_expression);
        }
        self.resolve_object_binding_from_expression(&Expression::Identifier("this".to_string()))
            .map(|binding| object_binding_to_expression(&binding))
            .or_else(|| {
                self.state
                    .speculation
                    .static_semantics
                    .local_value_binding("this")
                    .cloned()
            })
            .or_else(|| {
                self.backend
                    .global_semantics
                    .values
                    .value_bindings
                    .get("this")
                    .cloned()
            })
            .unwrap_or_else(|| self.materialize_static_expression(this_expression))
    }
}
