use super::*;

#[derive(Default)]
pub(in crate::backend::direct_wasm) struct ArgumentsUsage {
    pub(in crate::backend::direct_wasm) indexed_slots: Vec<u32>,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct ArgumentsIndexedPropertyState {
    pub(in crate::backend::direct_wasm) present: bool,
    pub(in crate::backend::direct_wasm) mapped: bool,
    pub(in crate::backend::direct_wasm) writable: bool,
    pub(in crate::backend::direct_wasm) enumerable: bool,
    pub(in crate::backend::direct_wasm) configurable: bool,
    pub(in crate::backend::direct_wasm) getter: Option<Expression>,
    pub(in crate::backend::direct_wasm) setter: Option<Expression>,
}

impl ArgumentsIndexedPropertyState {
    pub(in crate::backend::direct_wasm) fn data(present: bool, mapped: bool) -> Self {
        Self {
            present,
            mapped,
            writable: true,
            enumerable: true,
            configurable: true,
            getter: None,
            setter: None,
        }
    }

    pub(in crate::backend::direct_wasm) fn is_accessor(&self) -> bool {
        self.getter.is_some() || self.setter.is_some()
    }
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct ArgumentsSlot {
    pub(in crate::backend::direct_wasm) value_local: u32,
    pub(in crate::backend::direct_wasm) present_local: u32,
    pub(in crate::backend::direct_wasm) mapped_local: Option<u32>,
    pub(in crate::backend::direct_wasm) source_param_local: Option<u32>,
    pub(in crate::backend::direct_wasm) state: ArgumentsIndexedPropertyState,
}
