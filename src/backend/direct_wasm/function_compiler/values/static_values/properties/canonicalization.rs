use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn canonical_object_property_expression(
        &self,
        property: &Expression,
    ) -> Expression {
        let materialized = self.materialize_static_expression(property);
        let coerced = self
            .resolve_property_key_expression(property)
            .unwrap_or(materialized);
        self.resolve_symbol_identity_expression(&coerced)
            .or_else(|| self.resolve_symbol_identity_expression(property))
            .unwrap_or(coerced)
    }
}
