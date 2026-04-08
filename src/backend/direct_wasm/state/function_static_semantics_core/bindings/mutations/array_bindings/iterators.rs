use super::super::super::super::FunctionStaticSemanticsState;
use crate::backend::direct_wasm::{ArrayIteratorBinding, IteratorStepBinding};

impl FunctionStaticSemanticsState {
    pub(in crate::backend::direct_wasm) fn set_local_array_iterator_binding(
        &mut self,
        name: &str,
        binding: ArrayIteratorBinding,
    ) {
        self.arrays.set_local_array_iterator_binding(name, binding);
    }

    pub(in crate::backend::direct_wasm) fn clear_local_array_iterator_binding(
        &mut self,
        name: &str,
    ) {
        self.arrays.clear_local_array_iterator_binding(name);
    }

    pub(in crate::backend::direct_wasm) fn set_local_iterator_step_binding(
        &mut self,
        name: &str,
        binding: IteratorStepBinding,
    ) {
        self.arrays.set_local_iterator_step_binding(name, binding);
    }

    pub(in crate::backend::direct_wasm) fn clear_local_iterator_step_binding(
        &mut self,
        name: &str,
    ) {
        self.arrays.clear_local_iterator_step_binding(name);
    }
}
