use super::*;

impl GlobalValueService {
    pub(in crate::backend::direct_wasm) fn resolve_identifier_expression(
        &self,
        name: &str,
    ) -> Option<Expression> {
        self.value_bindings.get(name).cloned()
    }

    pub(in crate::backend::direct_wasm) fn find_identifier_binding_name(
        &self,
        identifier: &str,
    ) -> Option<String> {
        self.value_bindings
            .iter()
            .find_map(|(binding_name, value)| match value {
                Expression::Identifier(function_name) if function_name == identifier => {
                    Some(binding_name.clone())
                }
                _ => None,
            })
    }

    pub(in crate::backend::direct_wasm) fn find_home_object_binding_name(
        &self,
        function_name: &str,
    ) -> Option<String> {
        self.value_bindings.iter().find_map(|(name, value)| {
            let Expression::Object(entries) = value else {
                return None;
            };
            entries.iter().find_map(|entry| {
                let candidate = match entry {
                    crate::ir::hir::ObjectEntry::Data { value, .. } => value,
                    crate::ir::hir::ObjectEntry::Getter { getter, .. } => getter,
                    crate::ir::hir::ObjectEntry::Setter { setter, .. } => setter,
                    crate::ir::hir::ObjectEntry::Spread(_) => return None,
                };
                matches!(candidate, Expression::Identifier(candidate_name) if candidate_name == function_name)
                    .then_some(name.clone())
            })
        })
    }
}
