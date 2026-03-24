use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn ensure_runtime_array_length_local(
        &mut self,
        name: &str,
    ) -> u32 {
        if let Some(local) = self.runtime_array_length_locals.get(name).copied() {
            return local;
        }
        let local = self.allocate_temp_local();
        self.runtime_array_length_locals
            .insert(name.to_string(), local);
        local
    }

    pub(in crate::backend::direct_wasm) fn resolve_runtime_array_binding_name(
        &self,
        name: &str,
    ) -> Option<String> {
        if self.local_array_bindings.contains_key(name)
            || self.runtime_array_length_locals.contains_key(name)
            || self.runtime_array_slots.contains_key(name)
        {
            return Some(name.to_string());
        }
        let (resolved_name, _) = self.resolve_current_local_binding(name)?;
        if self.local_array_bindings.contains_key(&resolved_name)
            || self
                .runtime_array_length_locals
                .contains_key(&resolved_name)
            || self.runtime_array_slots.contains_key(&resolved_name)
        {
            return Some(resolved_name);
        }
        None
    }

    pub(in crate::backend::direct_wasm) fn resolve_local_array_iterator_binding_name(
        &self,
        name: &str,
    ) -> Option<String> {
        if self.local_array_iterator_bindings.contains_key(name) {
            return Some(name.to_string());
        }
        let (resolved_name, _) = self.resolve_current_local_binding(name)?;
        self.local_array_iterator_bindings
            .contains_key(&resolved_name)
            .then_some(resolved_name)
    }

    pub(in crate::backend::direct_wasm) fn runtime_array_length_local_for_expression(
        &self,
        expression: &Expression,
    ) -> Option<u32> {
        let Expression::Identifier(name) = expression else {
            return None;
        };
        let binding_name = self
            .resolve_runtime_array_binding_name(name)
            .unwrap_or_else(|| name.clone());
        self.runtime_array_length_locals.get(&binding_name).copied()
    }

    pub(in crate::backend::direct_wasm) fn ensure_runtime_array_slots_for_binding(
        &mut self,
        name: &str,
        binding: &ArrayValueBinding,
    ) {
        for index in 0..TRACKED_ARRAY_SLOT_LIMIT {
            let slot = if let Some(slot) = self.runtime_array_slot(name, index) {
                slot
            } else {
                let slot = RuntimeArraySlot {
                    value_local: self.allocate_temp_local(),
                    present_local: self.allocate_temp_local(),
                };
                self.runtime_array_slots
                    .entry(name.to_string())
                    .or_default()
                    .insert(index, slot.clone());
                slot
            };
            match binding.values.get(index as usize).cloned().flatten() {
                Some(value) => {
                    self.emit_numeric_expression(&value)
                        .expect("runtime array slot initialization is supported");
                    self.push_local_set(slot.value_local);
                    self.push_i32_const(1);
                    self.push_local_set(slot.present_local);
                }
                None => {
                    self.push_i32_const(JS_UNDEFINED_TAG);
                    self.push_local_set(slot.value_local);
                    self.push_i32_const(0);
                    self.push_local_set(slot.present_local);
                }
            }
        }
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
        self.instructions.push(0x04);
        self.instructions.push(I32_TYPE);
        self.push_control_frame();
        self.push_local_get(slot.value_local);
        self.instructions.push(0x05);
        self.push_i32_const(JS_UNDEFINED_TAG);
        self.instructions.push(0x0b);
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
        if let Some(length_local) = self.runtime_array_length_locals.get(&binding_name).copied() {
            let next_length = index as i32 + 1;
            self.push_local_get(length_local);
            self.push_i32_const(next_length);
            self.push_binary_op(BinaryOp::LessThan)?;
            self.instructions.push(0x04);
            self.instructions.push(EMPTY_BLOCK_TYPE);
            self.push_control_frame();
            self.push_i32_const(next_length);
            self.push_local_set(length_local);
            self.instructions.push(0x0b);
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

    pub(in crate::backend::direct_wasm) fn is_named_global_array_binding(
        &self,
        name: &str,
    ) -> bool {
        self.resolve_current_local_binding(name).is_none()
            && self.module.global_array_bindings.contains_key(name)
    }

    pub(in crate::backend::direct_wasm) fn uses_global_runtime_array_state(
        &self,
        name: &str,
    ) -> bool {
        self.module.global_arrays_with_runtime_state.contains(name)
    }

    pub(in crate::backend::direct_wasm) fn global_runtime_array_length_binding_name(
        &self,
        name: &str,
    ) -> String {
        format!("__ayy_global_array_length_{name}")
    }

    pub(in crate::backend::direct_wasm) fn global_runtime_array_slot_binding_name(
        &self,
        name: &str,
        index: u32,
    ) -> String {
        format!("__ayy_global_array_slot_{name}_{index}")
    }

    pub(in crate::backend::direct_wasm) fn global_runtime_array_length_binding(
        &mut self,
        name: &str,
    ) -> ImplicitGlobalBinding {
        let hidden_name = self.global_runtime_array_length_binding_name(name);
        self.module.ensure_implicit_global_binding(&hidden_name)
    }

    pub(in crate::backend::direct_wasm) fn global_runtime_array_slot_binding(
        &mut self,
        name: &str,
        index: u32,
    ) -> ImplicitGlobalBinding {
        let hidden_name = self.global_runtime_array_slot_binding_name(name, index);
        self.module.ensure_implicit_global_binding(&hidden_name)
    }

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
        if !self.top_level_function {
            self.module
                .global_arrays_with_runtime_state
                .insert(name.to_string());
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
        self.instructions.push(0x04);
        self.instructions.push(I32_TYPE);
        self.push_control_frame();
        self.push_global_get(binding.value_index);
        self.instructions.push(0x05);
        self.push_i32_const(JS_UNDEFINED_TAG);
        self.instructions.push(0x0b);
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
            self.instructions.push(0x04);
            self.instructions.push(I32_TYPE);
            self.push_control_frame();
            open_frames += 1;
            self.emit_global_runtime_array_slot_read(name, index)?;
            self.instructions.push(0x05);
        }
        self.push_i32_const(JS_UNDEFINED_TAG);
        for _ in 0..open_frames {
            self.instructions.push(0x0b);
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
        if !self.top_level_function {
            self.module
                .global_arrays_with_runtime_state
                .insert(name.to_string());
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
        self.instructions.push(0x04);
        self.instructions.push(EMPTY_BLOCK_TYPE);
        self.push_control_frame();
        self.push_i32_const(next_length);
        self.push_global_set(length_binding.value_index);
        self.push_i32_const(1);
        self.push_global_set(length_binding.present_index);
        self.instructions.push(0x0b);
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
        if !self.top_level_function {
            self.module
                .global_arrays_with_runtime_state
                .insert(name.to_string());
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
            self.instructions.push(0x04);
            self.instructions.push(I32_TYPE);
            self.push_control_frame();
            open_frames += 1;
            self.emit_global_runtime_array_slot_write_from_local(name, index, value_local)?;
            self.instructions.push(0x05);
        }

        self.push_local_get(value_local);
        for _ in 0..open_frames {
            self.instructions.push(0x0b);
            self.pop_control_frame();
        }
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_sync_global_runtime_array_state_from_binding(
        &mut self,
        name: &str,
        binding: &ArrayValueBinding,
    ) -> DirectResult<bool> {
        if !self.is_named_global_array_binding(name) {
            return Ok(false);
        }
        if self.top_level_function {
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

    pub(in crate::backend::direct_wasm) fn emit_dynamic_runtime_array_slot_write(
        &mut self,
        name: &str,
        property: &Expression,
        value: &Expression,
    ) -> DirectResult<bool> {
        let binding_name = self
            .resolve_runtime_array_binding_name(name)
            .unwrap_or_else(|| name.to_string());
        if !self.runtime_array_length_locals.contains_key(&binding_name)
            && !self.local_array_bindings.contains_key(&binding_name)
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
            self.instructions.push(0x04);
            self.instructions.push(I32_TYPE);
            self.push_control_frame();
            open_frames += 1;
            self.update_tracked_array_specialized_function_value(&binding_name, index, value)?;
            if !self.emit_runtime_array_slot_write_from_local(&binding_name, index, value_local)? {
                self.push_local_get(value_local);
            }
            self.instructions.push(0x05);
        }

        self.push_local_get(value_local);
        for _ in 0..open_frames {
            self.instructions.push(0x0b);
            self.pop_control_frame();
        }
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_runtime_array_push_from_local(
        &mut self,
        name: &str,
        value_local: u32,
        value_expression: &Expression,
    ) -> DirectResult<bool> {
        let binding_name = self
            .resolve_runtime_array_binding_name(name)
            .unwrap_or_else(|| name.to_string());
        let Some(length_local) = self.runtime_array_length_locals.get(&binding_name).copied()
        else {
            return Ok(false);
        };
        if binding_name.starts_with("__ayy_array_rest_")
            && let Expression::Member { object, property } = value_expression
            && matches!(property.as_ref(), Expression::String(property_name) if property_name == "value")
            && let Some(IteratorStepBinding::Runtime { done_local, .. }) =
                self.resolve_iterator_step_binding_from_expression(object)
        {
            self.push_local_get(done_local);
            self.instructions.push(0x04);
            self.instructions.push(I32_TYPE);
            self.push_control_frame();
            self.push_local_get(length_local);
            self.instructions.push(0x05);
            self.emit_runtime_array_push_with_length_local(
                &binding_name,
                length_local,
                value_local,
                value_expression,
            )?;
            self.instructions.push(0x0b);
            self.pop_control_frame();
            return Ok(true);
        }
        self.emit_runtime_array_push_with_length_local(
            &binding_name,
            length_local,
            value_local,
            value_expression,
        )
    }

    pub(in crate::backend::direct_wasm) fn emit_runtime_array_push_with_length_local(
        &mut self,
        name: &str,
        length_local: u32,
        value_local: u32,
        value_expression: &Expression,
    ) -> DirectResult<bool> {
        let Some(indices) = self
            .runtime_array_slots
            .get(name)
            .map(|slots| slots.keys().copied().collect::<Vec<_>>())
        else {
            self.push_local_get(length_local);
            self.push_i32_const(1);
            self.push_binary_op(BinaryOp::Add)?;
            self.push_local_tee(length_local);
            return Ok(true);
        };

        let mut sorted_indices = indices;
        sorted_indices.sort_unstable();
        let mut open_frames = 0;
        let matched_local = self.allocate_temp_local();
        self.push_i32_const(0);
        self.push_local_set(matched_local);
        for index in sorted_indices {
            self.push_local_get(length_local);
            self.push_i32_const(index as i32);
            self.push_binary_op(BinaryOp::Equal)?;
            self.instructions.push(0x04);
            self.instructions.push(EMPTY_BLOCK_TYPE);
            self.push_control_frame();
            open_frames += 1;
            self.update_tracked_array_specialized_function_value(name, index, value_expression)?;
            if self.emit_runtime_array_slot_write_from_local(name, index, value_local)? {
                self.instructions.push(0x1a);
            }
            self.push_i32_const(1);
            self.push_local_set(matched_local);
            self.instructions.push(0x05);
        }
        self.push_local_get(matched_local);
        self.instructions.push(0x04);
        self.instructions.push(EMPTY_BLOCK_TYPE);
        self.push_control_frame();
        self.instructions.push(0x05);
        self.push_local_get(length_local);
        self.push_i32_const(1);
        self.push_binary_op(BinaryOp::Add)?;
        self.push_local_set(length_local);
        self.instructions.push(0x0b);
        self.pop_control_frame();
        for _ in 0..open_frames {
            self.instructions.push(0x0b);
            self.pop_control_frame();
        }
        self.push_local_get(length_local);
        Ok(true)
    }
}
