use super::super::super::super::FunctionStaticSemanticsState;
use crate::backend::direct_wasm::{RuntimeArraySlot, SpecializedFunctionValue};
use std::collections::HashMap;

impl FunctionStaticSemanticsState {
    pub(in crate::backend::direct_wasm) fn runtime_array_length_local(
        &self,
        name: &str,
    ) -> Option<u32> {
        self.arrays.runtime_array_length_local(name)
    }

    pub(in crate::backend::direct_wasm) fn has_runtime_array_slots(&self, name: &str) -> bool {
        self.arrays.has_runtime_array_slots(name)
    }

    pub(in crate::backend::direct_wasm) fn runtime_array_slot(
        &self,
        name: &str,
        index: u32,
    ) -> Option<RuntimeArraySlot> {
        self.arrays.runtime_array_slot(name, index)
    }

    pub(in crate::backend::direct_wasm) fn runtime_array_slot_indices(
        &self,
        name: &str,
    ) -> Vec<u32> {
        self.arrays.runtime_array_slot_indices(name)
    }

    pub(in crate::backend::direct_wasm) fn runtime_array_slots(
        &self,
        name: &str,
    ) -> Option<HashMap<u32, RuntimeArraySlot>> {
        self.arrays.runtime_array_slots(name)
    }

    pub(in crate::backend::direct_wasm) fn tracked_array_specialized_function_value(
        &self,
        name: &str,
        index: u32,
    ) -> Option<SpecializedFunctionValue> {
        self.arrays
            .tracked_array_specialized_function_value(name, index)
    }

    pub(in crate::backend::direct_wasm) fn tracked_array_specialized_function_values(
        &self,
        name: &str,
    ) -> Option<HashMap<u32, SpecializedFunctionValue>> {
        self.arrays.tracked_array_specialized_function_values(name)
    }
}
