use super::super::super::super::FunctionStaticSemanticsState;
use crate::backend::direct_wasm::{RuntimeArraySlot, SpecializedFunctionValue};
use std::collections::HashMap;

impl FunctionStaticSemanticsState {
    pub(in crate::backend::direct_wasm) fn set_runtime_array_length_local(
        &mut self,
        name: &str,
        local: u32,
    ) {
        self.arrays.set_runtime_array_length_local(name, local);
    }

    pub(in crate::backend::direct_wasm) fn clear_runtime_array_length_local(&mut self, name: &str) {
        self.arrays.clear_runtime_array_length_local(name);
    }

    pub(in crate::backend::direct_wasm) fn set_runtime_array_slot(
        &mut self,
        name: &str,
        index: u32,
        slot: RuntimeArraySlot,
    ) {
        self.arrays.set_runtime_array_slot(name, index, slot);
    }

    pub(in crate::backend::direct_wasm) fn set_runtime_array_slots(
        &mut self,
        name: &str,
        slots: HashMap<u32, RuntimeArraySlot>,
    ) {
        self.arrays.set_runtime_array_slots(name, slots);
    }

    pub(in crate::backend::direct_wasm) fn clear_runtime_array_slots(&mut self, name: &str) {
        self.arrays.clear_runtime_array_slots(name);
    }

    pub(in crate::backend::direct_wasm) fn set_tracked_array_specialized_function_value(
        &mut self,
        name: &str,
        index: u32,
        value: SpecializedFunctionValue,
    ) {
        self.arrays
            .set_tracked_array_specialized_function_value(name, index, value);
    }

    pub(in crate::backend::direct_wasm) fn clear_tracked_array_specialized_function_value(
        &mut self,
        name: &str,
        index: u32,
    ) {
        self.arrays
            .clear_tracked_array_specialized_function_value(name, index);
    }

    pub(in crate::backend::direct_wasm) fn set_tracked_array_specialized_function_values(
        &mut self,
        name: &str,
        values: HashMap<u32, SpecializedFunctionValue>,
    ) {
        self.arrays
            .set_tracked_array_specialized_function_values(name, values);
    }

    pub(in crate::backend::direct_wasm) fn clear_tracked_array_specialized_function_values(
        &mut self,
        name: &str,
    ) {
        self.arrays
            .clear_tracked_array_specialized_function_values(name);
    }
}
