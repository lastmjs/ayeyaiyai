use super::*;

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct PropertyDescriptorBinding {
    pub(in crate::backend::direct_wasm) value: Option<Expression>,
    pub(in crate::backend::direct_wasm) configurable: bool,
    pub(in crate::backend::direct_wasm) enumerable: bool,
    pub(in crate::backend::direct_wasm) writable: Option<bool>,
    pub(in crate::backend::direct_wasm) getter: Option<Expression>,
    pub(in crate::backend::direct_wasm) setter: Option<Expression>,
    pub(in crate::backend::direct_wasm) has_get: bool,
    pub(in crate::backend::direct_wasm) has_set: bool,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct GlobalPropertyDescriptorState {
    pub(in crate::backend::direct_wasm) value: Expression,
    pub(in crate::backend::direct_wasm) writable: Option<bool>,
    pub(in crate::backend::direct_wasm) enumerable: bool,
    pub(in crate::backend::direct_wasm) configurable: bool,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) enum StringConcatFragment {
    Static(String),
    Dynamic(Expression),
}

#[derive(Clone, Default)]
pub(in crate::backend::direct_wasm) struct PropertyDescriptorDefinition {
    pub(in crate::backend::direct_wasm) value: Option<Expression>,
    pub(in crate::backend::direct_wasm) writable: Option<bool>,
    pub(in crate::backend::direct_wasm) enumerable: Option<bool>,
    pub(in crate::backend::direct_wasm) configurable: Option<bool>,
    pub(in crate::backend::direct_wasm) getter: Option<Expression>,
    pub(in crate::backend::direct_wasm) setter: Option<Expression>,
}

impl PropertyDescriptorDefinition {
    pub(in crate::backend::direct_wasm) fn is_accessor(&self) -> bool {
        self.getter.is_some() || self.setter.is_some()
    }
}
