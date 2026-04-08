use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_global_runtime_array_length_read(
        &mut self,
        name: &str,
    ) -> bool {
        if !self.is_named_global_array_binding(name) || !self.uses_global_runtime_array_state(name)
        {
            return false;
        }
        let binding = self.global_runtime_array_length_binding(name);
        self.push_global_get(binding.value_index);
        true
    }

    pub(in crate::backend::direct_wasm) fn emit_global_runtime_array_length_write(
        &mut self,
        name: &str,
        length: i32,
    ) -> bool {
        if !self.is_named_global_array_binding(name) {
            return false;
        }
        if !self.state.speculation.execution_context.top_level_function {
            self.backend.mark_global_array_with_runtime_state(name);
        }
        let binding = self.global_runtime_array_length_binding(name);
        self.push_i32_const(length);
        self.push_global_set(binding.value_index);
        self.push_i32_const(1);
        self.push_global_set(binding.present_index);
        true
    }

    pub(in crate::backend::direct_wasm) fn emit_global_runtime_array_slot_read(
        &mut self,
        name: &str,
        index: u32,
    ) -> DirectResult<bool> {
        if !self.is_named_global_array_binding(name) || !self.uses_global_runtime_array_state(name)
        {
            return Ok(false);
        }
        let binding = self.global_runtime_array_slot_binding(name, index);
        self.push_global_get(binding.present_index);
        self.state.emission.output.instructions.push(0x04);
        self.state.emission.output.instructions.push(I32_TYPE);
        self.push_control_frame();
        self.push_global_get(binding.value_index);
        self.state.emission.output.instructions.push(0x05);
        self.push_i32_const(JS_UNDEFINED_TAG);
        self.state.emission.output.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_dynamic_global_runtime_array_slot_read_from_local(
        &mut self,
        name: &str,
        index_local: u32,
    ) -> DirectResult<bool> {
        if !self.is_named_global_array_binding(name) {
            return Ok(false);
        }
        let mut open_frames = 0;
        for index in 0..TRACKED_ARRAY_SLOT_LIMIT {
            self.push_local_get(index_local);
            self.push_i32_const(index as i32);
            self.push_binary_op(BinaryOp::Equal)?;
            self.state.emission.output.instructions.push(0x04);
            self.state.emission.output.instructions.push(I32_TYPE);
            self.push_control_frame();
            open_frames += 1;
            self.emit_global_runtime_array_slot_read(name, index)?;
            self.state.emission.output.instructions.push(0x05);
        }
        self.push_i32_const(JS_UNDEFINED_TAG);
        for _ in 0..open_frames {
            self.state.emission.output.instructions.push(0x0b);
            self.pop_control_frame();
        }
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_global_runtime_array_slot_write_from_local(
        &mut self,
        name: &str,
        index: u32,
        value_local: u32,
    ) -> DirectResult<bool> {
        if !self.is_named_global_array_binding(name) {
            return Ok(false);
        }
        if !self.state.speculation.execution_context.top_level_function {
            self.backend.mark_global_array_with_runtime_state(name);
        }
        let binding = self.global_runtime_array_slot_binding(name, index);
        self.push_local_get(value_local);
        self.push_global_set(binding.value_index);
        self.push_i32_const(1);
        self.push_global_set(binding.present_index);

        let length_binding = self.global_runtime_array_length_binding(name);
        let next_length = index as i32 + 1;
        self.push_global_get(length_binding.value_index);
        self.push_i32_const(next_length);
        self.push_binary_op(BinaryOp::LessThan)?;
        self.state.emission.output.instructions.push(0x04);
        self.state
            .emission
            .output
            .instructions
            .push(EMPTY_BLOCK_TYPE);
        self.push_control_frame();
        self.push_i32_const(next_length);
        self.push_global_set(length_binding.value_index);
        self.push_i32_const(1);
        self.push_global_set(length_binding.present_index);
        self.state.emission.output.instructions.push(0x0b);
        self.pop_control_frame();

        self.push_local_get(value_local);
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn clear_global_runtime_array_slot(
        &mut self,
        name: &str,
        index: u32,
    ) -> bool {
        if !self.is_named_global_array_binding(name) {
            return false;
        }
        if !self.state.speculation.execution_context.top_level_function {
            self.backend.mark_global_array_with_runtime_state(name);
        }
        let binding = self.global_runtime_array_slot_binding(name, index);
        self.push_i32_const(JS_UNDEFINED_TAG);
        self.push_global_set(binding.value_index);
        self.push_i32_const(0);
        self.push_global_set(binding.present_index);
        true
    }

    pub(in crate::backend::direct_wasm) fn emit_dynamic_global_runtime_array_slot_write(
        &mut self,
        name: &str,
        property: &Expression,
        value: &Expression,
    ) -> DirectResult<bool> {
        if !self.is_named_global_array_binding(name) {
            return Ok(false);
        }

        let property_local = self.allocate_temp_local();
        self.emit_numeric_expression(property)?;
        self.push_local_set(property_local);

        let value_local = self.allocate_temp_local();
        self.emit_numeric_expression(value)?;
        self.push_local_set(value_local);

        let mut open_frames = 0;
        for index in 0..TRACKED_ARRAY_SLOT_LIMIT {
            self.push_local_get(property_local);
            self.push_i32_const(index as i32);
            self.push_binary_op(BinaryOp::Equal)?;
            self.state.emission.output.instructions.push(0x04);
            self.state.emission.output.instructions.push(I32_TYPE);
            self.push_control_frame();
            open_frames += 1;
            self.emit_global_runtime_array_slot_write_from_local(name, index, value_local)?;
            self.state.emission.output.instructions.push(0x05);
        }

        self.push_local_get(value_local);
        for _ in 0..open_frames {
            self.state.emission.output.instructions.push(0x0b);
            self.pop_control_frame();
        }
        Ok(true)
    }
}
