use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_sync_global_runtime_array_state_from_binding(
        &mut self,
        name: &str,
        binding: &ArrayValueBinding,
    ) -> DirectResult<bool> {
        if !self.is_named_global_array_binding(name) {
            return Ok(false);
        }
        if self.state.speculation.execution_context.top_level_function
            && !self.uses_global_runtime_array_state(name)
        {
            let length_local = self.ensure_runtime_array_length_local(name);
            self.push_i32_const(binding.values.len() as i32);
            self.push_local_set(length_local);
            self.ensure_runtime_array_slots_for_binding(name, binding);
            return Ok(true);
        }

        self.emit_global_runtime_array_length_write(name, binding.values.len() as i32);
        for index in 0..TRACKED_ARRAY_SLOT_LIMIT {
            let slot_binding = self.global_runtime_array_slot_binding(name, index);
            match binding.values.get(index as usize).cloned().flatten() {
                Some(value) => {
                    self.emit_numeric_expression(&value)?;
                    self.push_global_set(slot_binding.value_index);
                    self.push_i32_const(1);
                    self.push_global_set(slot_binding.present_index);
                }
                None => {
                    self.push_i32_const(JS_UNDEFINED_TAG);
                    self.push_global_set(slot_binding.value_index);
                    self.push_i32_const(0);
                    self.push_global_set(slot_binding.present_index);
                }
            }
        }
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_force_global_runtime_array_state_from_binding(
        &mut self,
        name: &str,
        binding: &ArrayValueBinding,
    ) -> DirectResult<bool> {
        if !self.is_named_global_array_binding(name) {
            return Ok(false);
        }
        self.backend.mark_global_array_with_runtime_state(name);
        self.emit_global_runtime_array_length_write(name, binding.values.len() as i32);
        for index in 0..TRACKED_ARRAY_SLOT_LIMIT {
            let slot_binding = self.global_runtime_array_slot_binding(name, index);
            match binding.values.get(index as usize).cloned().flatten() {
                Some(value) => {
                    self.emit_numeric_expression(&value)?;
                    self.push_global_set(slot_binding.value_index);
                    self.push_i32_const(1);
                    self.push_global_set(slot_binding.present_index);
                }
                None => {
                    self.push_i32_const(JS_UNDEFINED_TAG);
                    self.push_global_set(slot_binding.value_index);
                    self.push_i32_const(0);
                    self.push_global_set(slot_binding.present_index);
                }
            }
        }
        Ok(true)
    }
}
