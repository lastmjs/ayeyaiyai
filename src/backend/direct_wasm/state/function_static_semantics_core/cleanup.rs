use super::*;

impl FunctionStaticSemanticsState {
    pub(in crate::backend::direct_wasm) fn clear_isolated_indirect_eval_state(&mut self) {
        self.values.clear_isolated_indirect_eval_state();
        self.objects.clear_isolated_indirect_eval_state();
        self.arrays.clear_isolated_indirect_eval_state();
        self.eval_lexical_initialized_locals.clear();
        self.capture_slot_source_bindings.clear();
        self.last_bound_user_function_call = None;
    }

    pub(in crate::backend::direct_wasm) fn clear_eval_local_function_binding_metadata(
        &mut self,
        name: &str,
    ) {
        self.values.clear_eval_local_function_binding_metadata(name);
    }

    pub(in crate::backend::direct_wasm) fn clear_local_static_binding_metadata(
        &mut self,
        name: &str,
    ) {
        self.values.clear_local_static_binding_metadata(name);
        self.arrays.clear_local_static_binding_metadata(name);
        self.objects.clear_local_binding_metadata(name);
    }

    pub(in crate::backend::direct_wasm) fn clear_local_runtime_binding_metadata(
        &mut self,
        name: &str,
    ) {
        self.values.clear_local_runtime_binding_metadata(name);
        self.arrays.clear_local_runtime_binding_metadata(name);
        self.objects.clear_local_binding_metadata(name);
    }

    pub(in crate::backend::direct_wasm) fn clear_member_bindings_for_name(
        &mut self,
        name: &str,
        include_prototype: bool,
    ) {
        self.objects
            .clear_member_bindings_for_name(name, include_prototype);
    }
}
