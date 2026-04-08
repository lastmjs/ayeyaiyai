use super::super::super::super::FunctionStaticSemanticsState;
use crate::backend::direct_wasm::ArrayValueBinding;

impl FunctionStaticSemanticsState {
    pub(in crate::backend::direct_wasm) fn set_local_array_binding(
        &mut self,
        name: &str,
        array: ArrayValueBinding,
    ) {
        self.arrays.set_local_array_binding(name, array);
    }

    pub(in crate::backend::direct_wasm) fn clear_local_array_binding(&mut self, name: &str) {
        self.arrays.clear_local_array_binding(name);
    }
}
