use super::*;

impl<'a> FunctionCompilerBackend<'a> {
    pub(in crate::backend::direct_wasm) fn sync_global_array_binding(
        &mut self,
        name: &str,
        binding: Option<ArrayValueBinding>,
    ) {
        self.global_semantics
            .values
            .sync_array_binding(name, binding);
    }

    pub(in crate::backend::direct_wasm) fn mark_global_array_with_runtime_state(
        &mut self,
        name: &str,
    ) {
        self.global_semantics
            .values
            .mark_array_with_runtime_state(name);
    }
}
