use super::FunctionArraySemanticsState;
use crate::backend::direct_wasm::ArrayValueBinding;

impl FunctionArraySemanticsState {
    pub(in crate::backend::direct_wasm) fn local_array_binding_mut(
        &mut self,
        name: &str,
    ) -> Option<&mut ArrayValueBinding> {
        self.local_array_bindings.get_mut(name)
    }

    pub(in crate::backend::direct_wasm) fn local_array_binding(
        &self,
        name: &str,
    ) -> Option<&ArrayValueBinding> {
        self.local_array_bindings.get(name)
    }

    pub(in crate::backend::direct_wasm) fn set_local_array_binding(
        &mut self,
        name: &str,
        array: ArrayValueBinding,
    ) {
        self.local_array_bindings.insert(name.to_string(), array);
    }

    pub(in crate::backend::direct_wasm) fn clear_local_array_binding(&mut self, name: &str) {
        self.local_array_bindings.remove(name);
    }
}
