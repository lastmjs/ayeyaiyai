use super::FunctionArraySemanticsState;
use crate::backend::direct_wasm::{ArrayIteratorBinding, IteratorStepBinding};

impl FunctionArraySemanticsState {
    pub(in crate::backend::direct_wasm) fn local_array_iterator_binding(
        &self,
        name: &str,
    ) -> Option<&ArrayIteratorBinding> {
        self.local_array_iterator_bindings.get(name)
    }

    pub(in crate::backend::direct_wasm) fn local_array_iterator_binding_mut(
        &mut self,
        name: &str,
    ) -> Option<&mut ArrayIteratorBinding> {
        self.local_array_iterator_bindings.get_mut(name)
    }

    pub(in crate::backend::direct_wasm) fn has_local_array_iterator_binding(
        &self,
        name: &str,
    ) -> bool {
        self.local_array_iterator_bindings.contains_key(name)
    }

    pub(in crate::backend::direct_wasm) fn set_local_array_iterator_binding(
        &mut self,
        name: &str,
        binding: ArrayIteratorBinding,
    ) {
        self.local_array_iterator_bindings
            .insert(name.to_string(), binding);
    }

    pub(in crate::backend::direct_wasm) fn clear_local_array_iterator_binding(
        &mut self,
        name: &str,
    ) {
        self.local_array_iterator_bindings.remove(name);
    }

    pub(in crate::backend::direct_wasm) fn local_iterator_step_binding(
        &self,
        name: &str,
    ) -> Option<&IteratorStepBinding> {
        self.local_iterator_step_bindings.get(name)
    }

    pub(in crate::backend::direct_wasm) fn set_local_iterator_step_binding(
        &mut self,
        name: &str,
        binding: IteratorStepBinding,
    ) {
        self.local_iterator_step_bindings
            .insert(name.to_string(), binding);
    }

    pub(in crate::backend::direct_wasm) fn clear_local_iterator_step_binding(
        &mut self,
        name: &str,
    ) {
        self.local_iterator_step_bindings.remove(name);
    }
}
