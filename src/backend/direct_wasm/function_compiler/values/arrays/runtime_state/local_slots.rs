use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_runtime_array_slot_read(
        &mut self,
        name: &str,
        index: u32,
    ) -> DirectResult<bool> {
        let binding_name = self
            .resolve_runtime_array_binding_name(name)
            .unwrap_or_else(|| name.to_string());
        let Some(slot) = self.runtime_array_slot(&binding_name, index) else {
            return Ok(false);
        };
        self.push_local_get(slot.present_local);
        self.state.emission.output.instructions.push(0x04);
        self.state.emission.output.instructions.push(I32_TYPE);
        self.push_control_frame();
        self.push_local_get(slot.value_local);
        self.state.emission.output.instructions.push(0x05);
        self.push_i32_const(JS_UNDEFINED_TAG);
        self.state.emission.output.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_runtime_array_slot_write_from_local(
        &mut self,
        name: &str,
        index: u32,
        value_local: u32,
    ) -> DirectResult<bool> {
        let binding_name = self
            .resolve_runtime_array_binding_name(name)
            .unwrap_or_else(|| name.to_string());
        let Some(slot) = self.runtime_array_slot(&binding_name, index) else {
            return Ok(false);
        };
        self.push_local_get(value_local);
        self.push_local_set(slot.value_local);
        self.push_i32_const(1);
        self.push_local_set(slot.present_local);
        if let Some(length_local) = self
            .state
            .speculation
            .static_semantics
            .runtime_array_length_local(&binding_name)
        {
            let next_length = index as i32 + 1;
            self.push_local_get(length_local);
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
            self.push_local_set(length_local);
            self.state.emission.output.instructions.push(0x0b);
            self.pop_control_frame();
        }
        self.push_local_get(value_local);
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn clear_runtime_array_slot(
        &mut self,
        name: &str,
        index: u32,
    ) -> bool {
        let binding_name = self
            .resolve_runtime_array_binding_name(name)
            .unwrap_or_else(|| name.to_string());
        let Some(slot) = self.runtime_array_slot(&binding_name, index) else {
            return false;
        };
        self.push_i32_const(JS_UNDEFINED_TAG);
        self.push_local_set(slot.value_local);
        self.push_i32_const(0);
        self.push_local_set(slot.present_local);
        true
    }

    pub(in crate::backend::direct_wasm) fn emit_dynamic_runtime_array_slot_write(
        &mut self,
        name: &str,
        property: &Expression,
        value: &Expression,
    ) -> DirectResult<bool> {
        let binding_name = self
            .resolve_runtime_array_binding_name(name)
            .unwrap_or_else(|| name.to_string());
        if !self
            .state
            .speculation
            .static_semantics
            .runtime_array_length_local(&binding_name)
            .is_some()
            && !self
                .state
                .speculation
                .static_semantics
                .has_local_array_binding(&binding_name)
        {
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
            self.ensure_runtime_array_slot_entry(&binding_name, index);
            self.push_local_get(property_local);
            self.push_i32_const(index as i32);
            self.push_binary_op(BinaryOp::Equal)?;
            self.state.emission.output.instructions.push(0x04);
            self.state.emission.output.instructions.push(I32_TYPE);
            self.push_control_frame();
            open_frames += 1;
            self.update_tracked_array_specialized_function_value(&binding_name, index, value)?;
            if !self.emit_runtime_array_slot_write_from_local(&binding_name, index, value_local)? {
                self.push_local_get(value_local);
            }
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
