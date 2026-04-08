use super::*;

impl CompilerState {
    pub(in crate::backend::direct_wasm) fn sync_global_object_binding(
        &mut self,
        name: &str,
        binding: Option<ObjectValueBinding>,
    ) {
        self.global_semantics
            .values
            .sync_object_binding(name, binding);
    }

    pub(in crate::backend::direct_wasm) fn sync_global_prototype_object_binding(
        &mut self,
        name: &str,
        binding: Option<ObjectValueBinding>,
    ) {
        self.global_semantics
            .values
            .sync_prototype_object_binding(name, binding);
    }
}
