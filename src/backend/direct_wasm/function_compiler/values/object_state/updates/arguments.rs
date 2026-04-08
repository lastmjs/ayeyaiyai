use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn update_local_arguments_binding(
        &mut self,
        name: &str,
        value: &Expression,
    ) {
        if self.is_direct_arguments_object(value) {
            self.state
                .parameters
                .direct_arguments_aliases
                .insert(name.to_string());
            self.state.parameters.local_arguments_bindings.remove(name);
            self.state
                .speculation
                .static_semantics
                .set_local_kind(name, StaticValueKind::Object);
            return;
        }
        self.state.parameters.direct_arguments_aliases.remove(name);
        let Some(arguments_binding) = self.resolve_arguments_binding_from_expression(value) else {
            self.state.parameters.local_arguments_bindings.remove(name);
            return;
        };
        self.state
            .parameters
            .local_arguments_bindings
            .insert(name.to_string(), arguments_binding);
        self.state
            .speculation
            .static_semantics
            .set_local_kind(name, StaticValueKind::Object);
    }
}
