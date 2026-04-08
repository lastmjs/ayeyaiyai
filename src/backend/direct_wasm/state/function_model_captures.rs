use super::super::*;

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct PreparedCaptureBinding {
    pub(in crate::backend::direct_wasm) binding: ImplicitGlobalBinding,
    pub(in crate::backend::direct_wasm) source_name: String,
    pub(in crate::backend::direct_wasm) hidden_name: String,
    pub(in crate::backend::direct_wasm) saved_value_local: u32,
    pub(in crate::backend::direct_wasm) saved_present_local: u32,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct PreparedBoundCaptureBinding {
    pub(in crate::backend::direct_wasm) binding: ImplicitGlobalBinding,
    pub(in crate::backend::direct_wasm) capture_name: String,
    pub(in crate::backend::direct_wasm) capture_hidden_name: String,
    pub(in crate::backend::direct_wasm) slot_name: String,
    pub(in crate::backend::direct_wasm) source_binding_name: Option<String>,
    pub(in crate::backend::direct_wasm) slot_local: u32,
    pub(in crate::backend::direct_wasm) saved_value_local: u32,
    pub(in crate::backend::direct_wasm) saved_present_local: u32,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct BoundUserFunctionCallSnapshot {
    pub(in crate::backend::direct_wasm) function_name: String,
    pub(in crate::backend::direct_wasm) source_expression: Option<Expression>,
    pub(in crate::backend::direct_wasm) result_expression: Option<Expression>,
    pub(in crate::backend::direct_wasm) updated_bindings: HashMap<String, Expression>,
}
