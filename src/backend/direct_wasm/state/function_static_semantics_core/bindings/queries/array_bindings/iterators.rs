use super::super::super::super::FunctionStaticSemanticsState;
use crate::backend::direct_wasm::{ArrayIteratorBinding, IteratorStepBinding};

impl FunctionStaticSemanticsState {
    pub(in crate::backend::direct_wasm) fn local_array_iterator_binding(
        &self,
        name: &str,
    ) -> Option<&ArrayIteratorBinding> {
        self.arrays.local_array_iterator_binding(name)
    }

    pub(in crate::backend::direct_wasm) fn local_array_iterator_binding_mut(
        &mut self,
        name: &str,
    ) -> Option<&mut ArrayIteratorBinding> {
        self.arrays.local_array_iterator_binding_mut(name)
    }

    pub(in crate::backend::direct_wasm) fn has_local_array_iterator_binding(
        &self,
        name: &str,
    ) -> bool {
        self.arrays.has_local_array_iterator_binding(name)
    }

    pub(in crate::backend::direct_wasm) fn local_iterator_step_binding(
        &self,
        name: &str,
    ) -> Option<&IteratorStepBinding> {
        self.arrays.local_iterator_step_binding(name)
    }
}
