use crate::backend::direct_wasm::GlobalValueService;

impl GlobalValueService {
    pub(in crate::backend::direct_wasm) fn reset_for_program(&mut self) {
        self.value_bindings.clear();
        self.array_bindings.clear();
        self.arrays_with_runtime_state.clear();
        self.object_bindings.clear();
        self.property_descriptors.clear();
        self.object_prototype_bindings.clear();
        self.runtime_prototype_bindings.clear();
        self.prototype_object_bindings.clear();
        self.arguments_bindings.clear();
        self.proxy_bindings.clear();
    }
}
