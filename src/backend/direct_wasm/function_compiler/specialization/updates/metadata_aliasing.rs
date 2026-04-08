use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn alias_runtime_binding_metadata(
        &mut self,
        target: &str,
        source: &str,
    ) {
        if let Some(function_binding) = self
            .state
            .speculation
            .static_semantics
            .local_function_binding(source)
            .cloned()
        {
            self.state
                .speculation
                .static_semantics
                .set_local_function_binding(target, function_binding);
        }
        if let Some(specialized) = self
            .state
            .speculation
            .static_semantics
            .values
            .local_specialized_function_values
            .get(source)
            .cloned()
        {
            self.state
                .speculation
                .static_semantics
                .values
                .local_specialized_function_values
                .insert(target.to_string(), specialized);
        }
        if let Some(array_binding) = self
            .state
            .speculation
            .static_semantics
            .local_array_binding(source)
            .cloned()
        {
            self.state
                .speculation
                .static_semantics
                .set_local_array_binding(target, array_binding);
        }
        if let Some(length_local) = self
            .state
            .speculation
            .static_semantics
            .runtime_array_length_local(source)
        {
            self.state
                .speculation
                .static_semantics
                .set_runtime_array_length_local(target, length_local);
        }
        if let Some(slots) = self
            .state
            .speculation
            .static_semantics
            .runtime_array_slots(source)
        {
            self.state
                .speculation
                .static_semantics
                .set_runtime_array_slots(target, slots);
        }
        if let Some(bindings) = self
            .state
            .speculation
            .static_semantics
            .tracked_array_specialized_function_values(source)
        {
            self.state
                .speculation
                .static_semantics
                .set_tracked_array_specialized_function_values(target, bindings);
        }
        if let Some(object_binding) = self
            .state
            .speculation
            .static_semantics
            .local_object_binding(source)
            .cloned()
        {
            self.state
                .speculation
                .static_semantics
                .set_local_object_binding(target, object_binding);
        }
        if let Some(arguments_binding) = self
            .state
            .parameters
            .local_arguments_bindings
            .get(source)
            .cloned()
        {
            self.state
                .parameters
                .local_arguments_bindings
                .insert(target.to_string(), arguments_binding);
        }
        if self
            .state
            .parameters
            .direct_arguments_aliases
            .contains(source)
        {
            self.state
                .parameters
                .direct_arguments_aliases
                .insert(target.to_string());
        }
        if let Some(descriptor) = self
            .state
            .speculation
            .static_semantics
            .objects
            .local_descriptor_bindings
            .get(source)
            .cloned()
        {
            self.state
                .speculation
                .static_semantics
                .objects
                .local_descriptor_bindings
                .insert(target.to_string(), descriptor);
        }
        if let Some(buffer_binding) = self
            .state
            .speculation
            .static_semantics
            .local_resizable_array_buffer_binding(source)
            .cloned()
        {
            self.state
                .speculation
                .static_semantics
                .set_local_resizable_array_buffer_binding(target, buffer_binding);
        }
        if let Some(view_binding) = self
            .state
            .speculation
            .static_semantics
            .local_typed_array_view_binding(source)
            .cloned()
        {
            self.state
                .speculation
                .static_semantics
                .set_local_typed_array_view_binding(target, view_binding);
        }
        if let Some(oob_local) = self
            .state
            .speculation
            .static_semantics
            .runtime_typed_array_oob_local(source)
        {
            self.state
                .speculation
                .static_semantics
                .set_runtime_typed_array_oob_local(target, oob_local);
        }
        if let Some(iterator_binding) = self
            .state
            .speculation
            .static_semantics
            .local_array_iterator_binding(source)
            .cloned()
        {
            self.state
                .speculation
                .static_semantics
                .set_local_array_iterator_binding(target, iterator_binding);
        }
        if let Some(step_binding) = self
            .state
            .speculation
            .static_semantics
            .local_iterator_step_binding(source)
            .cloned()
        {
            self.state
                .speculation
                .static_semantics
                .set_local_iterator_step_binding(target, step_binding);
        }
        if let Some(kind) = self.state.speculation.static_semantics.local_kind(source) {
            self.state
                .speculation
                .static_semantics
                .set_local_kind(target, kind);
        }
    }
}
