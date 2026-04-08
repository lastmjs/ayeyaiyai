use super::super::super::super::FunctionStaticSemanticsState;
use crate::backend::direct_wasm::{ResizableArrayBufferBinding, TypedArrayViewBinding};

impl FunctionStaticSemanticsState {
    pub(in crate::backend::direct_wasm) fn set_local_typed_array_view_binding(
        &mut self,
        name: &str,
        binding: TypedArrayViewBinding,
    ) {
        self.arrays
            .set_local_typed_array_view_binding(name, binding);
    }

    pub(in crate::backend::direct_wasm) fn clear_local_typed_array_view_binding(
        &mut self,
        name: &str,
    ) {
        self.arrays.clear_local_typed_array_view_binding(name);
    }

    pub(in crate::backend::direct_wasm) fn set_local_resizable_array_buffer_binding(
        &mut self,
        name: &str,
        binding: ResizableArrayBufferBinding,
    ) {
        self.arrays
            .set_local_resizable_array_buffer_binding(name, binding);
    }

    pub(in crate::backend::direct_wasm) fn clear_local_resizable_array_buffer_binding(
        &mut self,
        name: &str,
    ) {
        self.arrays.clear_local_resizable_array_buffer_binding(name);
    }

    pub(in crate::backend::direct_wasm) fn set_runtime_typed_array_oob_local(
        &mut self,
        name: &str,
        local: u32,
    ) {
        self.arrays.set_runtime_typed_array_oob_local(name, local);
    }

    pub(in crate::backend::direct_wasm) fn clear_runtime_typed_array_oob_local(
        &mut self,
        name: &str,
    ) {
        self.arrays.clear_runtime_typed_array_oob_local(name);
    }
}
