use super::*;

impl CompilerState {
    pub(in crate::backend::direct_wasm) fn sync_global_array_binding(
        &mut self,
        name: &str,
        binding: Option<ArrayValueBinding>,
    ) {
        self.global_semantics
            .values
            .sync_array_binding(name, binding);
    }

    pub(in crate::backend::direct_wasm) fn set_global_array_element_binding(
        &mut self,
        name: &str,
        index: usize,
        value: Expression,
    ) -> bool {
        self.global_semantics
            .values
            .set_array_element_binding(name, index, value)
    }
}
