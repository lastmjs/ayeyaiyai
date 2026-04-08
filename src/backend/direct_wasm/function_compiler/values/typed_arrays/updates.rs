use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn apply_resizable_array_buffer_resize(
        &mut self,
        name: &str,
        new_length: usize,
    ) -> DirectResult<bool> {
        let Some(binding) = self
            .state
            .speculation
            .static_semantics
            .local_resizable_array_buffer_binding_mut(name)
        else {
            return Ok(false);
        };
        if new_length > binding.max_length {
            return self.emit_named_error_throw("RangeError").map(|_| true);
        }
        let old_length = binding.values.len();
        if new_length < old_length {
            binding.values.truncate(new_length);
        } else if new_length > old_length {
            binding
                .values
                .extend((old_length..new_length).map(|_| Some(Expression::Number(0.0))));
        }

        let length_local = self.ensure_runtime_array_length_local(name);
        self.push_i32_const(new_length as i32);
        self.push_local_set(length_local);
        for index in 0..TRACKED_ARRAY_SLOT_LIMIT {
            let slot = self.ensure_runtime_array_slot_entry(name, index);
            if index < new_length as u32 {
                if index >= old_length as u32 {
                    self.push_i32_const(0);
                    self.push_local_set(slot.value_local);
                    self.push_i32_const(1);
                    self.push_local_set(slot.present_local);
                }
            } else {
                self.push_i32_const(JS_UNDEFINED_TAG);
                self.push_local_set(slot.value_local);
                self.push_i32_const(0);
                self.push_local_set(slot.present_local);
            }
        }
        self.sync_typed_array_views_for_buffer(name)?;
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_typed_array_view_write(
        &mut self,
        view_name: &str,
        property: &Expression,
        value: &Expression,
    ) -> DirectResult<bool> {
        let Some(view) = self
            .state
            .speculation
            .static_semantics
            .local_typed_array_view_binding(view_name)
            .cloned()
        else {
            return Ok(false);
        };
        let value_local = self.allocate_temp_local();
        self.emit_numeric_expression(value)?;
        self.push_local_set(value_local);

        let handled = if let Some(index) = argument_index_from_expression(property) {
            let buffer_index = view.offset + index as usize;
            let materialized = self.materialize_static_expression(value);
            if let Some(buffer) = self
                .state
                .speculation
                .static_semantics
                .local_resizable_array_buffer_binding_mut(&view.buffer_name)
            {
                if buffer_index < buffer.values.len() {
                    buffer.values[buffer_index] = Some(materialized);
                }
            }
            self.emit_runtime_array_slot_write_from_local(
                &view.buffer_name,
                buffer_index as u32,
                value_local,
            )?
        } else if view.offset == 0 {
            self.emit_dynamic_runtime_array_slot_write(&view.buffer_name, property, value)?
        } else {
            false
        };

        if handled {
            self.state.emission.output.instructions.push(0x1a);
            self.sync_typed_array_views_for_buffer(&view.buffer_name)?;
            self.push_local_get(value_local);
            return Ok(true);
        }

        self.push_local_get(value_local);
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn update_local_resizable_array_buffer_binding(
        &mut self,
        name: &str,
        value: &Expression,
    ) -> DirectResult<()> {
        let Some((length, max_length)) = self.resolve_array_buffer_binding_from_expression(value)
        else {
            self.state
                .speculation
                .static_semantics
                .clear_local_resizable_array_buffer_binding(name);
            return Ok(());
        };
        let binding = ResizableArrayBufferBinding {
            values: vec![Some(Expression::Number(0.0)); length],
            max_length,
        };
        let runtime_binding = ArrayValueBinding {
            values: binding.values.clone(),
        };
        self.state
            .speculation
            .static_semantics
            .set_local_resizable_array_buffer_binding(name, binding);
        let length_local = self.ensure_runtime_array_length_local(name);
        self.push_i32_const(length as i32);
        self.push_local_set(length_local);
        self.ensure_runtime_array_slots_for_binding(name, &runtime_binding);
        self.state
            .speculation
            .static_semantics
            .set_local_kind(name, StaticValueKind::Object);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn update_local_typed_array_view_binding(
        &mut self,
        name: &str,
        value: &Expression,
    ) -> DirectResult<()> {
        let Some(binding) = self.resolve_typed_array_view_binding_from_expression(value) else {
            self.state
                .speculation
                .static_semantics
                .clear_local_typed_array_view_binding(name);
            self.state
                .speculation
                .static_semantics
                .clear_runtime_typed_array_oob_local(name);
            return Ok(());
        };
        self.state
            .speculation
            .static_semantics
            .set_local_typed_array_view_binding(name, binding);
        self.sync_typed_array_view_runtime_state(name)
    }
}
