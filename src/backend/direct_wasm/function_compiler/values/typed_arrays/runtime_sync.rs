use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn sync_typed_array_view_runtime_state(
        &mut self,
        name: &str,
    ) -> DirectResult<()> {
        let Some(view) = self
            .state
            .speculation
            .static_semantics
            .local_typed_array_view_binding(name)
            .cloned()
        else {
            return Ok(());
        };
        let Some(buffer_length_local) = self
            .state
            .speculation
            .static_semantics
            .runtime_array_length_local(&view.buffer_name)
        else {
            return Ok(());
        };
        let view_length_local = self.ensure_runtime_array_length_local(name);
        let oob_local = self.typed_array_oob_local(name);
        let max_tracked = view
            .fixed_length
            .unwrap_or(TRACKED_ARRAY_SLOT_LIMIT as usize);
        let tracked_limit = max_tracked.min(TRACKED_ARRAY_SLOT_LIMIT as usize) as u32;

        match view.fixed_length {
            Some(length) => {
                self.push_local_get(buffer_length_local);
                self.push_i32_const((view.offset + length) as i32);
                self.push_binary_op(BinaryOp::LessThan)?;
                self.push_local_set(oob_local);

                self.push_local_get(oob_local);
                self.state.emission.output.instructions.push(0x04);
                self.state
                    .emission
                    .output
                    .instructions
                    .push(EMPTY_BLOCK_TYPE);
                self.push_control_frame();
                self.push_i32_const(0);
                self.push_local_set(view_length_local);
                for index in 0..tracked_limit {
                    let slot = self.ensure_runtime_array_slot_entry(name, index);
                    self.push_i32_const(JS_UNDEFINED_TAG);
                    self.push_local_set(slot.value_local);
                    self.push_i32_const(0);
                    self.push_local_set(slot.present_local);
                }
                self.state.emission.output.instructions.push(0x05);
                self.push_i32_const(length as i32);
                self.push_local_set(view_length_local);
                for index in 0..tracked_limit {
                    let slot = self.ensure_runtime_array_slot_entry(name, index);
                    let buffer_slot = self.ensure_runtime_array_slot_entry(
                        &view.buffer_name,
                        view.offset as u32 + index,
                    );
                    self.push_local_get(buffer_slot.value_local);
                    self.push_local_set(slot.value_local);
                    self.push_local_get(buffer_slot.present_local);
                    self.push_local_set(slot.present_local);
                }
                self.state.emission.output.instructions.push(0x0b);
                self.pop_control_frame();
            }
            None => {
                self.push_local_get(buffer_length_local);
                self.push_i32_const(view.offset as i32);
                self.push_binary_op(BinaryOp::LessThan)?;
                self.push_local_set(oob_local);

                self.push_local_get(oob_local);
                self.state.emission.output.instructions.push(0x04);
                self.state
                    .emission
                    .output
                    .instructions
                    .push(EMPTY_BLOCK_TYPE);
                self.push_control_frame();
                self.push_i32_const(0);
                self.push_local_set(view_length_local);
                for index in 0..tracked_limit {
                    let slot = self.ensure_runtime_array_slot_entry(name, index);
                    self.push_i32_const(JS_UNDEFINED_TAG);
                    self.push_local_set(slot.value_local);
                    self.push_i32_const(0);
                    self.push_local_set(slot.present_local);
                }
                self.state.emission.output.instructions.push(0x05);
                self.push_local_get(buffer_length_local);
                self.push_i32_const(view.offset as i32);
                self.push_binary_op(BinaryOp::Subtract)?;
                self.push_local_set(view_length_local);
                for index in 0..tracked_limit {
                    let slot = self.ensure_runtime_array_slot_entry(name, index);
                    self.push_local_get(view_length_local);
                    self.push_i32_const(index as i32);
                    self.push_binary_op(BinaryOp::GreaterThan)?;
                    self.state.emission.output.instructions.push(0x04);
                    self.state
                        .emission
                        .output
                        .instructions
                        .push(EMPTY_BLOCK_TYPE);
                    self.push_control_frame();
                    let buffer_slot = self.ensure_runtime_array_slot_entry(
                        &view.buffer_name,
                        view.offset as u32 + index,
                    );
                    self.push_local_get(buffer_slot.value_local);
                    self.push_local_set(slot.value_local);
                    self.push_local_get(buffer_slot.present_local);
                    self.push_local_set(slot.present_local);
                    self.state.emission.output.instructions.push(0x05);
                    self.push_i32_const(JS_UNDEFINED_TAG);
                    self.push_local_set(slot.value_local);
                    self.push_i32_const(0);
                    self.push_local_set(slot.present_local);
                    self.state.emission.output.instructions.push(0x0b);
                    self.pop_control_frame();
                }
                self.state.emission.output.instructions.push(0x0b);
                self.pop_control_frame();
            }
        }

        if let Some(values) = self.typed_array_view_static_values(&view) {
            self.state
                .speculation
                .static_semantics
                .set_local_array_binding(name, values);
        } else {
            self.state
                .speculation
                .static_semantics
                .clear_local_array_binding(name);
        }
        self.state
            .speculation
            .static_semantics
            .set_local_kind(name, StaticValueKind::Object);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn sync_typed_array_views_for_buffer(
        &mut self,
        buffer_name: &str,
    ) -> DirectResult<()> {
        let names = self
            .state
            .speculation
            .static_semantics
            .typed_array_view_binding_names_for_buffer(buffer_name);
        for name in names {
            self.sync_typed_array_view_runtime_state(&name)?;
        }
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_dynamic_runtime_array_slot_read_from_local(
        &mut self,
        name: &str,
        index_local: u32,
    ) -> DirectResult<bool> {
        let Some(indices) = self
            .state
            .speculation
            .static_semantics
            .has_runtime_array_slots(name)
            .then(|| {
                self.state
                    .speculation
                    .static_semantics
                    .runtime_array_slot_indices(name)
            })
        else {
            return Ok(false);
        };

        let mut sorted_indices = indices;
        sorted_indices.sort_unstable();
        let mut open_frames = 0;
        for index in sorted_indices {
            self.push_local_get(index_local);
            self.push_i32_const(index as i32);
            self.push_binary_op(BinaryOp::Equal)?;
            self.state.emission.output.instructions.push(0x04);
            self.state.emission.output.instructions.push(I32_TYPE);
            self.push_control_frame();
            open_frames += 1;
            if !self.emit_runtime_array_slot_read(name, index)? {
                self.push_i32_const(JS_UNDEFINED_TAG);
            }
            self.state.emission.output.instructions.push(0x05);
        }
        self.push_i32_const(JS_UNDEFINED_TAG);
        for _ in 0..open_frames {
            self.state.emission.output.instructions.push(0x0b);
            self.pop_control_frame();
        }
        Ok(true)
    }
}
