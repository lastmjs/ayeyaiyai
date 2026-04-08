use super::super::super::super::FunctionStaticSemanticsState;
use crate::backend::direct_wasm::{ResizableArrayBufferBinding, TypedArrayViewBinding};

impl FunctionStaticSemanticsState {
    pub(in crate::backend::direct_wasm) fn local_typed_array_view_binding(
        &self,
        name: &str,
    ) -> Option<&TypedArrayViewBinding> {
        self.arrays.local_typed_array_view_binding(name)
    }

    pub(in crate::backend::direct_wasm) fn has_local_typed_array_view_binding(
        &self,
        name: &str,
    ) -> bool {
        self.arrays.has_local_typed_array_view_binding(name)
    }

    pub(in crate::backend::direct_wasm) fn typed_array_view_binding_names_for_buffer(
        &self,
        buffer_name: &str,
    ) -> Vec<String> {
        self.arrays
            .typed_array_view_binding_names_for_buffer(buffer_name)
    }

    pub(in crate::backend::direct_wasm) fn local_resizable_array_buffer_binding(
        &self,
        name: &str,
    ) -> Option<&ResizableArrayBufferBinding> {
        self.arrays.local_resizable_array_buffer_binding(name)
    }

    pub(in crate::backend::direct_wasm) fn local_resizable_array_buffer_binding_mut(
        &mut self,
        name: &str,
    ) -> Option<&mut ResizableArrayBufferBinding> {
        self.arrays.local_resizable_array_buffer_binding_mut(name)
    }

    pub(in crate::backend::direct_wasm) fn runtime_typed_array_oob_local(
        &self,
        name: &str,
    ) -> Option<u32> {
        self.arrays.runtime_typed_array_oob_local(name)
    }
}
