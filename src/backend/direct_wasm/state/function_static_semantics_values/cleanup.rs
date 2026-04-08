use super::FunctionValueSemanticsState;

impl FunctionValueSemanticsState {
    pub(in crate::backend::direct_wasm) fn clear_isolated_indirect_eval_state(&mut self) {
        self.local_kinds.clear();
        self.local_value_bindings.clear();
        self.local_function_bindings.clear();
        self.local_specialized_function_values.clear();
        self.local_proxy_bindings.clear();
    }

    pub(in crate::backend::direct_wasm) fn clear_eval_local_function_binding_metadata(
        &mut self,
        name: &str,
    ) {
        self.local_value_bindings.remove(name);
        self.local_function_bindings.remove(name);
        self.local_kinds.remove(name);
    }

    pub(in crate::backend::direct_wasm) fn clear_local_static_binding_metadata(
        &mut self,
        name: &str,
    ) {
        self.local_value_bindings.remove(name);
        self.local_function_bindings.remove(name);
        self.local_kinds.remove(name);
        self.local_proxy_bindings.remove(name);
        self.local_specialized_function_values.remove(name);
    }

    pub(in crate::backend::direct_wasm) fn clear_local_runtime_binding_metadata(
        &mut self,
        name: &str,
    ) {
        self.clear_local_static_binding_metadata(name);
    }
}
