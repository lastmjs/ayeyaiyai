use super::*;

impl FunctionCompilerState {
    pub(in crate::backend::direct_wasm) fn clear_eval_local_function_binding_metadata(
        &mut self,
        name: &str,
    ) {
        self.speculation
            .static_semantics
            .clear_eval_local_function_binding_metadata(name);
    }

    pub(in crate::backend::direct_wasm) fn clear_local_static_binding_metadata(
        &mut self,
        name: &str,
    ) {
        self.speculation
            .static_semantics
            .clear_local_static_binding_metadata(name);
        self.parameters.clear_local_binding_metadata(name);
    }

    pub(in crate::backend::direct_wasm) fn clear_local_runtime_binding_metadata(
        &mut self,
        name: &str,
    ) {
        self.runtime.locals.remove(name);
        self.speculation
            .static_semantics
            .clear_local_runtime_binding_metadata(name);
        self.parameters.clear_local_binding_metadata(name);
    }

    pub(in crate::backend::direct_wasm) fn clear_member_bindings_for_name(
        &mut self,
        name: &str,
        include_prototype: bool,
    ) {
        self.speculation
            .static_semantics
            .clear_member_bindings_for_name(name, include_prototype);
    }
}
