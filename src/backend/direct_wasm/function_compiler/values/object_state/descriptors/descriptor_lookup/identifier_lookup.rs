use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_identifier_descriptor_binding(
        &self,
        name: &str,
    ) -> Option<PropertyDescriptorBinding> {
        let resolved_name = self
            .resolve_current_local_binding(name)
            .map(|(resolved_name, _)| resolved_name)
            .unwrap_or_else(|| name.to_string());
        self.state
            .speculation
            .static_semantics
            .objects
            .local_descriptor_bindings
            .get(&resolved_name)
            .cloned()
    }
}
