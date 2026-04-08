use super::FunctionArraySemanticsState;
use crate::backend::direct_wasm::{RuntimeArraySlot, SpecializedFunctionValue};
use std::collections::HashMap;

impl FunctionArraySemanticsState {
    pub(in crate::backend::direct_wasm) fn runtime_array_length_local(
        &self,
        name: &str,
    ) -> Option<u32> {
        self.runtime_array_length_locals.get(name).copied()
    }

    pub(in crate::backend::direct_wasm) fn set_runtime_array_length_local(
        &mut self,
        name: &str,
        local: u32,
    ) {
        self.runtime_array_length_locals
            .insert(name.to_string(), local);
    }

    pub(in crate::backend::direct_wasm) fn clear_runtime_array_length_local(&mut self, name: &str) {
        self.runtime_array_length_locals.remove(name);
    }

    pub(in crate::backend::direct_wasm) fn has_runtime_array_slots(&self, name: &str) -> bool {
        self.runtime_array_slots.contains_key(name)
    }

    pub(in crate::backend::direct_wasm) fn runtime_array_slot(
        &self,
        name: &str,
        index: u32,
    ) -> Option<RuntimeArraySlot> {
        self.runtime_array_slots
            .get(name)
            .and_then(|slots| slots.get(&index))
            .cloned()
    }

    pub(in crate::backend::direct_wasm) fn runtime_array_slot_indices(
        &self,
        name: &str,
    ) -> Vec<u32> {
        self.runtime_array_slots
            .get(name)
            .map(|slots| slots.keys().copied().collect())
            .unwrap_or_default()
    }

    pub(in crate::backend::direct_wasm) fn runtime_array_slots(
        &self,
        name: &str,
    ) -> Option<HashMap<u32, RuntimeArraySlot>> {
        self.runtime_array_slots.get(name).cloned()
    }

    pub(in crate::backend::direct_wasm) fn set_runtime_array_slot(
        &mut self,
        name: &str,
        index: u32,
        slot: RuntimeArraySlot,
    ) {
        self.runtime_array_slots
            .entry(name.to_string())
            .or_default()
            .insert(index, slot);
    }

    pub(in crate::backend::direct_wasm) fn set_runtime_array_slots(
        &mut self,
        name: &str,
        slots: HashMap<u32, RuntimeArraySlot>,
    ) {
        self.runtime_array_slots.insert(name.to_string(), slots);
    }

    pub(in crate::backend::direct_wasm) fn clear_runtime_array_slots(&mut self, name: &str) {
        self.runtime_array_slots.remove(name);
    }

    pub(in crate::backend::direct_wasm) fn tracked_array_specialized_function_value(
        &self,
        name: &str,
        index: u32,
    ) -> Option<SpecializedFunctionValue> {
        self.tracked_array_function_values
            .get(name)
            .and_then(|bindings| bindings.get(&index))
            .cloned()
    }

    pub(in crate::backend::direct_wasm) fn tracked_array_specialized_function_values(
        &self,
        name: &str,
    ) -> Option<HashMap<u32, SpecializedFunctionValue>> {
        self.tracked_array_function_values.get(name).cloned()
    }

    pub(in crate::backend::direct_wasm) fn set_tracked_array_specialized_function_value(
        &mut self,
        name: &str,
        index: u32,
        value: SpecializedFunctionValue,
    ) {
        self.tracked_array_function_values
            .entry(name.to_string())
            .or_default()
            .insert(index, value);
    }

    pub(in crate::backend::direct_wasm) fn clear_tracked_array_specialized_function_value(
        &mut self,
        name: &str,
        index: u32,
    ) {
        if let Some(bindings) = self.tracked_array_function_values.get_mut(name) {
            bindings.remove(&index);
            if bindings.is_empty() {
                self.tracked_array_function_values.remove(name);
            }
        }
    }

    pub(in crate::backend::direct_wasm) fn set_tracked_array_specialized_function_values(
        &mut self,
        name: &str,
        values: HashMap<u32, SpecializedFunctionValue>,
    ) {
        self.tracked_array_function_values
            .insert(name.to_string(), values);
    }

    pub(in crate::backend::direct_wasm) fn clear_tracked_array_specialized_function_values(
        &mut self,
        name: &str,
    ) {
        self.tracked_array_function_values.remove(name);
    }
}
