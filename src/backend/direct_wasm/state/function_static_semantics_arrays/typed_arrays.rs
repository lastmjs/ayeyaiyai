use super::FunctionArraySemanticsState;
use crate::backend::direct_wasm::{ResizableArrayBufferBinding, TypedArrayViewBinding};

impl FunctionArraySemanticsState {
    pub(in crate::backend::direct_wasm) fn local_typed_array_view_binding(
        &self,
        name: &str,
    ) -> Option<&TypedArrayViewBinding> {
        self.local_typed_array_view_bindings.get(name)
    }

    pub(in crate::backend::direct_wasm) fn has_local_typed_array_view_binding(
        &self,
        name: &str,
    ) -> bool {
        self.local_typed_array_view_bindings.contains_key(name)
    }

    pub(in crate::backend::direct_wasm) fn set_local_typed_array_view_binding(
        &mut self,
        name: &str,
        binding: TypedArrayViewBinding,
    ) {
        self.local_typed_array_view_bindings
            .insert(name.to_string(), binding);
    }

    pub(in crate::backend::direct_wasm) fn clear_local_typed_array_view_binding(
        &mut self,
        name: &str,
    ) {
        self.local_typed_array_view_bindings.remove(name);
    }

    pub(in crate::backend::direct_wasm) fn local_resizable_array_buffer_binding(
        &self,
        name: &str,
    ) -> Option<&ResizableArrayBufferBinding> {
        self.local_resizable_array_buffer_bindings.get(name)
    }

    pub(in crate::backend::direct_wasm) fn local_resizable_array_buffer_binding_mut(
        &mut self,
        name: &str,
    ) -> Option<&mut ResizableArrayBufferBinding> {
        self.local_resizable_array_buffer_bindings.get_mut(name)
    }

    pub(in crate::backend::direct_wasm) fn set_local_resizable_array_buffer_binding(
        &mut self,
        name: &str,
        binding: ResizableArrayBufferBinding,
    ) {
        self.local_resizable_array_buffer_bindings
            .insert(name.to_string(), binding);
    }

    pub(in crate::backend::direct_wasm) fn clear_local_resizable_array_buffer_binding(
        &mut self,
        name: &str,
    ) {
        self.local_resizable_array_buffer_bindings.remove(name);
    }

    pub(in crate::backend::direct_wasm) fn typed_array_view_binding_names_for_buffer(
        &self,
        buffer_name: &str,
    ) -> Vec<String> {
        self.local_typed_array_view_bindings
            .iter()
            .filter_map(|(name, view)| (view.buffer_name == buffer_name).then_some(name.clone()))
            .collect()
    }

    pub(in crate::backend::direct_wasm) fn runtime_typed_array_oob_local(
        &self,
        name: &str,
    ) -> Option<u32> {
        self.runtime_typed_array_oob_locals.get(name).copied()
    }

    pub(in crate::backend::direct_wasm) fn set_runtime_typed_array_oob_local(
        &mut self,
        name: &str,
        local: u32,
    ) {
        self.runtime_typed_array_oob_locals
            .insert(name.to_string(), local);
    }

    pub(in crate::backend::direct_wasm) fn clear_runtime_typed_array_oob_local(
        &mut self,
        name: &str,
    ) {
        self.runtime_typed_array_oob_locals.remove(name);
    }
}
