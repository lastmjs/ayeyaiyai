use super::FunctionArraySemanticsState;

impl FunctionArraySemanticsState {
    pub(in crate::backend::direct_wasm) fn clear_isolated_indirect_eval_state(&mut self) {
        self.local_array_bindings.clear();
        self.local_resizable_array_buffer_bindings.clear();
        self.local_typed_array_view_bindings.clear();
        self.runtime_typed_array_oob_locals.clear();
        self.tracked_array_function_values.clear();
        self.runtime_array_slots.clear();
        self.local_array_iterator_bindings.clear();
        self.local_iterator_step_bindings.clear();
        self.runtime_array_length_locals.clear();
    }

    pub(in crate::backend::direct_wasm) fn clear_local_static_binding_metadata(
        &mut self,
        name: &str,
    ) {
        self.local_array_bindings.remove(name);
    }

    pub(in crate::backend::direct_wasm) fn clear_local_runtime_binding_metadata(
        &mut self,
        name: &str,
    ) {
        self.local_array_bindings.remove(name);
        self.local_resizable_array_buffer_bindings.remove(name);
        self.local_typed_array_view_bindings.remove(name);
        self.runtime_typed_array_oob_locals.remove(name);
        self.tracked_array_function_values.remove(name);
        self.runtime_array_slots.remove(name);
        self.local_array_iterator_bindings.remove(name);
        self.local_iterator_step_bindings.remove(name);
        self.runtime_array_length_locals.remove(name);
    }
}
