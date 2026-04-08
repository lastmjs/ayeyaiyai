use super::super::super::*;
use super::function_bindings::LocalFunctionBinding;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::backend::direct_wasm) enum ReturnedMemberFunctionBindingTarget {
    Value,
    Prototype,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub(in crate::backend::direct_wasm) struct ReturnedMemberFunctionBindingKey {
    pub(in crate::backend::direct_wasm) target: ReturnedMemberFunctionBindingTarget,
    pub(in crate::backend::direct_wasm) property: String,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct ReturnedMemberFunctionBinding {
    pub(in crate::backend::direct_wasm) target: ReturnedMemberFunctionBindingTarget,
    pub(in crate::backend::direct_wasm) property: String,
    pub(in crate::backend::direct_wasm) binding: LocalFunctionBinding,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct ReturnedMemberValueBinding {
    pub(in crate::backend::direct_wasm) property: String,
    pub(in crate::backend::direct_wasm) value: Expression,
}
