use super::super::super::*;

#[derive(Clone, Debug, PartialEq)]
pub(in crate::backend::direct_wasm) enum LocalFunctionBinding {
    User(String),
    Builtin(String),
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct ResolvedPropertyKey {
    pub(in crate::backend::direct_wasm) key: Expression,
    pub(in crate::backend::direct_wasm) coercion: Option<LocalFunctionBinding>,
}
