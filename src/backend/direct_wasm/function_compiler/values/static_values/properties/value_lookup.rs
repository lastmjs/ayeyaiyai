use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_object_binding_property_value(
        &self,
        object_binding: &ObjectValueBinding,
        property: &Expression,
    ) -> Option<Expression> {
        let canonical_property = self.canonical_object_property_expression(property);
        if let Some(value) = object_binding_lookup_value(object_binding, &canonical_property) {
            return Some(value.clone());
        }

        let requested_symbol = self
            .resolve_symbol_identity_expression(&canonical_property)
            .or_else(|| self.resolve_symbol_identity_expression(property))?;
        object_binding
            .symbol_properties
            .iter()
            .find_map(|(existing_key, value)| {
                let canonical_existing = self
                    .resolve_symbol_identity_expression(existing_key)
                    .unwrap_or_else(|| existing_key.clone());
                (static_expression_matches(&canonical_existing, &requested_symbol)
                    || static_expression_matches(existing_key, &requested_symbol))
                .then(|| value.clone())
            })
    }
}
