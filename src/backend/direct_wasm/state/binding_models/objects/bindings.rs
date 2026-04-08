use super::*;

#[derive(Clone, PartialEq)]
pub(in crate::backend::direct_wasm) struct ObjectValueBinding {
    pub(in crate::backend::direct_wasm) string_properties: Vec<(String, Expression)>,
    pub(in crate::backend::direct_wasm) symbol_properties: Vec<(Expression, Expression)>,
    pub(in crate::backend::direct_wasm) non_enumerable_string_properties: Vec<String>,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct ProxyValueBinding {
    pub(in crate::backend::direct_wasm) target: Expression,
    pub(in crate::backend::direct_wasm) has_binding: Option<LocalFunctionBinding>,
}
