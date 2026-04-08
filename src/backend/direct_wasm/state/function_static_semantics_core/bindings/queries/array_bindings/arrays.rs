use super::super::super::super::FunctionStaticSemanticsState;
use crate::backend::direct_wasm::ArrayValueBinding;

impl FunctionStaticSemanticsState {
    pub(in crate::backend::direct_wasm) fn local_array_binding_mut(
        &mut self,
        name: &str,
    ) -> Option<&mut ArrayValueBinding> {
        self.arrays.local_array_binding_mut(name)
    }

    pub(in crate::backend::direct_wasm) fn local_array_binding(
        &self,
        name: &str,
    ) -> Option<&ArrayValueBinding> {
        self.arrays.local_array_binding(name)
    }

    pub(in crate::backend::direct_wasm) fn has_local_array_binding(&self, name: &str) -> bool {
        self.local_array_binding(name).is_some()
    }
}
