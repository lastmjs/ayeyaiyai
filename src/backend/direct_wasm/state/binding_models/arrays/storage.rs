use super::*;

#[derive(Clone, PartialEq)]
pub(in crate::backend::direct_wasm) struct ArrayValueBinding {
    pub(in crate::backend::direct_wasm) values: Vec<Option<Expression>>,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct RuntimeArraySlot {
    pub(in crate::backend::direct_wasm) value_local: u32,
    pub(in crate::backend::direct_wasm) present_local: u32,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct ResizableArrayBufferBinding {
    pub(in crate::backend::direct_wasm) values: Vec<Option<Expression>>,
    pub(in crate::backend::direct_wasm) max_length: usize,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct TypedArrayViewBinding {
    pub(in crate::backend::direct_wasm) buffer_name: String,
    pub(in crate::backend::direct_wasm) offset: usize,
    pub(in crate::backend::direct_wasm) fixed_length: Option<usize>,
}
