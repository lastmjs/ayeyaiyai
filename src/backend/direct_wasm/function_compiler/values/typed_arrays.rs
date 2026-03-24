use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn ensure_runtime_array_slot_entry(
        &mut self,
        name: &str,
        index: u32,
    ) -> RuntimeArraySlot {
        if let Some(slot) = self.runtime_array_slot(name, index) {
            return slot;
        }
        let slot = RuntimeArraySlot {
            value_local: self.allocate_temp_local(),
            present_local: self.allocate_temp_local(),
        };
        self.runtime_array_slots
            .entry(name.to_string())
            .or_default()
            .insert(index, slot.clone());
        slot
    }

    pub(in crate::backend::direct_wasm) fn typed_array_oob_local(&mut self, name: &str) -> u32 {
        if let Some(local) = self.runtime_typed_array_oob_locals.get(name).copied() {
            return local;
        }
        let local = self.allocate_temp_local();
        self.runtime_typed_array_oob_locals
            .insert(name.to_string(), local);
        local
    }

    pub(in crate::backend::direct_wasm) fn resolve_array_buffer_binding_from_expression(
        &self,
        expression: &Expression,
    ) -> Option<(usize, usize)> {
        if let Expression::Identifier(name) = expression {
            let binding = self.local_resizable_array_buffer_bindings.get(name)?;
            return Some((binding.values.len(), binding.max_length));
        }

        let (callee, arguments) = match expression {
            Expression::New { callee, arguments } => (callee.as_ref(), arguments.as_slice()),
            Expression::Call { callee, arguments } => {
                if !matches!(callee.as_ref(), Expression::Identifier(_)) {
                    return None;
                }
                let resolved = self.resolve_static_call_result_expression(callee, arguments)?;
                return self.resolve_array_buffer_binding_from_expression(&resolved);
            }
            _ => return None,
        };

        if !matches!(callee, Expression::Identifier(name) if name == "ArrayBuffer") {
            return None;
        }

        let length = extract_typed_array_element_count(match arguments.first()? {
            CallArgument::Expression(expression) | CallArgument::Spread(expression) => expression,
        })?;

        let max_length = arguments
            .get(1)
            .and_then(|argument| match argument {
                CallArgument::Expression(Expression::Object(entries))
                | CallArgument::Spread(Expression::Object(entries)) => {
                    entries.iter().find_map(|entry| {
                        let crate::ir::hir::ObjectEntry::Data { key, value } = entry else {
                            return None;
                        };
                        if !matches!(key, Expression::String(name) if name == "maxByteLength") {
                            return None;
                        }
                        extract_typed_array_element_count(value)
                    })
                }
                _ => None,
            })
            .unwrap_or(length);

        Some((length, max_length))
    }

    pub(in crate::backend::direct_wasm) fn resolve_typed_array_view_binding_from_expression(
        &self,
        expression: &Expression,
    ) -> Option<TypedArrayViewBinding> {
        if let Expression::Identifier(name) = expression {
            return self.local_typed_array_view_bindings.get(name).cloned();
        }

        let arguments = match expression {
            Expression::New { arguments, .. } => arguments.as_slice(),
            Expression::Call { callee, arguments } => {
                if !matches!(callee.as_ref(), Expression::Identifier(_)) {
                    return None;
                }
                let resolved = self.resolve_static_call_result_expression(callee, arguments)?;
                return self.resolve_typed_array_view_binding_from_expression(&resolved);
            }
            _ => return None,
        };
        let buffer_expression = match arguments.first()? {
            CallArgument::Expression(expression) | CallArgument::Spread(expression) => expression,
        };
        let Expression::Identifier(buffer_name) = buffer_expression else {
            return None;
        };
        if !self
            .local_resizable_array_buffer_bindings
            .contains_key(buffer_name)
        {
            return None;
        }

        let offset = arguments
            .get(1)
            .and_then(|argument| match argument {
                CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                    extract_typed_array_element_count(expression)
                }
            })
            .unwrap_or(0);
        let fixed_length = arguments.get(2).and_then(|argument| match argument {
            CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                extract_typed_array_element_count(expression)
            }
        });

        Some(TypedArrayViewBinding {
            buffer_name: buffer_name.clone(),
            offset,
            fixed_length,
        })
    }

    pub(in crate::backend::direct_wasm) fn typed_array_view_static_length(
        &self,
        view: &TypedArrayViewBinding,
    ) -> Option<usize> {
        let buffer = self
            .local_resizable_array_buffer_bindings
            .get(&view.buffer_name)?;
        match view.fixed_length {
            Some(length) => {
                if view.offset + length > buffer.values.len() {
                    None
                } else {
                    Some(length)
                }
            }
            None => {
                if view.offset > buffer.values.len() {
                    None
                } else {
                    Some(buffer.values.len().saturating_sub(view.offset))
                }
            }
        }
    }

    pub(in crate::backend::direct_wasm) fn typed_array_view_static_values(
        &self,
        view: &TypedArrayViewBinding,
    ) -> Option<ArrayValueBinding> {
        let buffer = self
            .local_resizable_array_buffer_bindings
            .get(&view.buffer_name)?;
        let length = self.typed_array_view_static_length(view)?;
        Some(ArrayValueBinding {
            values: buffer.values[view.offset..view.offset + length].to_vec(),
        })
    }

    pub(in crate::backend::direct_wasm) fn sync_typed_array_view_runtime_state(
        &mut self,
        name: &str,
    ) -> DirectResult<()> {
        let Some(view) = self.local_typed_array_view_bindings.get(name).cloned() else {
            return Ok(());
        };
        let Some(buffer_length_local) = self
            .runtime_array_length_locals
            .get(&view.buffer_name)
            .copied()
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
                self.instructions.push(0x04);
                self.instructions.push(EMPTY_BLOCK_TYPE);
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
                self.instructions.push(0x05);
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
                self.instructions.push(0x0b);
                self.pop_control_frame();
            }
            None => {
                self.push_local_get(buffer_length_local);
                self.push_i32_const(view.offset as i32);
                self.push_binary_op(BinaryOp::LessThan)?;
                self.push_local_set(oob_local);

                self.push_local_get(oob_local);
                self.instructions.push(0x04);
                self.instructions.push(EMPTY_BLOCK_TYPE);
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
                self.instructions.push(0x05);
                self.push_local_get(buffer_length_local);
                self.push_i32_const(view.offset as i32);
                self.push_binary_op(BinaryOp::Subtract)?;
                self.push_local_set(view_length_local);
                for index in 0..tracked_limit {
                    let slot = self.ensure_runtime_array_slot_entry(name, index);
                    self.push_local_get(view_length_local);
                    self.push_i32_const(index as i32);
                    self.push_binary_op(BinaryOp::GreaterThan)?;
                    self.instructions.push(0x04);
                    self.instructions.push(EMPTY_BLOCK_TYPE);
                    self.push_control_frame();
                    let buffer_slot = self.ensure_runtime_array_slot_entry(
                        &view.buffer_name,
                        view.offset as u32 + index,
                    );
                    self.push_local_get(buffer_slot.value_local);
                    self.push_local_set(slot.value_local);
                    self.push_local_get(buffer_slot.present_local);
                    self.push_local_set(slot.present_local);
                    self.instructions.push(0x05);
                    self.push_i32_const(JS_UNDEFINED_TAG);
                    self.push_local_set(slot.value_local);
                    self.push_i32_const(0);
                    self.push_local_set(slot.present_local);
                    self.instructions.push(0x0b);
                    self.pop_control_frame();
                }
                self.instructions.push(0x0b);
                self.pop_control_frame();
            }
        }

        if let Some(values) = self.typed_array_view_static_values(&view) {
            self.local_array_bindings.insert(name.to_string(), values);
        } else {
            self.local_array_bindings.remove(name);
        }
        self.local_kinds
            .insert(name.to_string(), StaticValueKind::Object);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn sync_typed_array_views_for_buffer(
        &mut self,
        buffer_name: &str,
    ) -> DirectResult<()> {
        let names = self
            .local_typed_array_view_bindings
            .iter()
            .filter_map(|(name, view)| (view.buffer_name == buffer_name).then_some(name.clone()))
            .collect::<Vec<_>>();
        for name in names {
            self.sync_typed_array_view_runtime_state(&name)?;
        }
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn apply_resizable_array_buffer_resize(
        &mut self,
        name: &str,
        new_length: usize,
    ) -> DirectResult<bool> {
        let Some(binding) = self.local_resizable_array_buffer_bindings.get_mut(name) else {
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
        let Some(view) = self.local_typed_array_view_bindings.get(view_name).cloned() else {
            return Ok(false);
        };
        let value_local = self.allocate_temp_local();
        self.emit_numeric_expression(value)?;
        self.push_local_set(value_local);

        let handled = if let Some(index) = argument_index_from_expression(property) {
            let buffer_index = view.offset + index as usize;
            let materialized = self.materialize_static_expression(value);
            if let Some(buffer) = self
                .local_resizable_array_buffer_bindings
                .get_mut(&view.buffer_name)
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
            self.instructions.push(0x1a);
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
            self.local_resizable_array_buffer_bindings.remove(name);
            return Ok(());
        };
        let binding = ResizableArrayBufferBinding {
            values: vec![Some(Expression::Number(0.0)); length],
            max_length,
        };
        let runtime_binding = ArrayValueBinding {
            values: binding.values.clone(),
        };
        self.local_resizable_array_buffer_bindings
            .insert(name.to_string(), binding);
        let length_local = self.ensure_runtime_array_length_local(name);
        self.push_i32_const(length as i32);
        self.push_local_set(length_local);
        self.ensure_runtime_array_slots_for_binding(name, &runtime_binding);
        self.local_kinds
            .insert(name.to_string(), StaticValueKind::Object);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn update_local_typed_array_view_binding(
        &mut self,
        name: &str,
        value: &Expression,
    ) -> DirectResult<()> {
        let Some(binding) = self.resolve_typed_array_view_binding_from_expression(value) else {
            self.local_typed_array_view_bindings.remove(name);
            self.runtime_typed_array_oob_locals.remove(name);
            return Ok(());
        };
        self.local_typed_array_view_bindings
            .insert(name.to_string(), binding);
        self.sync_typed_array_view_runtime_state(name)
    }

    pub(in crate::backend::direct_wasm) fn emit_dynamic_runtime_array_slot_read_from_local(
        &mut self,
        name: &str,
        index_local: u32,
    ) -> DirectResult<bool> {
        let Some(indices) = self
            .runtime_array_slots
            .get(name)
            .map(|slots| slots.keys().copied().collect::<Vec<_>>())
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
            self.instructions.push(0x04);
            self.instructions.push(I32_TYPE);
            self.push_control_frame();
            open_frames += 1;
            if !self.emit_runtime_array_slot_read(name, index)? {
                self.push_i32_const(JS_UNDEFINED_TAG);
            }
            self.instructions.push(0x05);
        }
        self.push_i32_const(JS_UNDEFINED_TAG);
        for _ in 0..open_frames {
            self.instructions.push(0x0b);
            self.pop_control_frame();
        }
        Ok(true)
    }
}
