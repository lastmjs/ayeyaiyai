use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn initialize_arguments_object(
        &mut self,
        statements: &[Statement],
    ) -> DirectResult<()> {
        let Some(actual_argument_count_local) = self.actual_argument_count_local else {
            return Ok(());
        };

        let arguments_usage = collect_arguments_usage_from_statements(statements);
        for index in arguments_usage.indexed_slots {
            let source_param_local = if index < self.visible_param_count {
                Some(index)
            } else {
                self.extra_argument_param_locals.get(&index).copied()
            };
            let visible_param = index < self.visible_param_count;
            let mapped_argument = self.mapped_arguments && visible_param;
            let value_local = self.allocate_temp_local();
            let present_local = self.allocate_temp_local();
            let mapped_local = if visible_param {
                Some(self.allocate_temp_local())
            } else {
                None
            };

            if let Some(source_param_local) = source_param_local {
                self.push_local_get(source_param_local);
            } else {
                self.push_i32_const(JS_UNDEFINED_TAG);
            }
            self.push_local_set(value_local);

            self.push_local_get(actual_argument_count_local);
            self.push_i32_const(index as i32);
            self.push_binary_op(BinaryOp::GreaterThan)?;
            self.push_local_set(present_local);

            if let Some(mapped_local) = mapped_local {
                if self.mapped_arguments {
                    self.push_local_get(present_local);
                } else {
                    self.push_i32_const(0);
                }
                self.push_local_set(mapped_local);
            }

            self.arguments_slots.insert(
                index,
                ArgumentsSlot {
                    value_local,
                    present_local,
                    mapped_local,
                    source_param_local: visible_param.then_some(index),
                    state: ArgumentsIndexedPropertyState::data(visible_param, mapped_argument),
                },
            );
        }

        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn ensure_arguments_slot(
        &mut self,
        index: u32,
    ) -> DirectResult<()> {
        if self.arguments_slots.contains_key(&index) {
            return Ok(());
        }

        let visible_param = index < self.visible_param_count;
        let mapped_argument = self.mapped_arguments && visible_param;
        let value_local = self.allocate_temp_local();
        let present_local = self.allocate_temp_local();
        let mapped_local = if visible_param {
            Some(self.allocate_temp_local())
        } else {
            None
        };

        if let Some(source_param_local) = if visible_param {
            Some(index)
        } else {
            self.extra_argument_param_locals.get(&index).copied()
        } {
            self.push_local_get(source_param_local);
        } else {
            self.push_i32_const(JS_UNDEFINED_TAG);
        }
        self.push_local_set(value_local);

        if let Some(actual_argument_count_local) = self.actual_argument_count_local {
            self.push_local_get(actual_argument_count_local);
            self.push_i32_const(index as i32);
            self.push_binary_op(BinaryOp::GreaterThan)?;
        } else {
            self.push_i32_const(0);
        }
        self.push_local_set(present_local);

        if let Some(mapped_local) = mapped_local {
            if self.mapped_arguments {
                self.push_local_get(present_local);
            } else {
                self.push_i32_const(0);
            }
            self.push_local_set(mapped_local);
        }

        self.arguments_slots.insert(
            index,
            ArgumentsSlot {
                value_local,
                present_local,
                mapped_local,
                source_param_local: visible_param.then_some(index),
                state: ArgumentsIndexedPropertyState::data(visible_param, mapped_argument),
            },
        );
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn has_arguments_object(&self) -> bool {
        self.actual_argument_count_local.is_some()
    }

    pub(in crate::backend::direct_wasm) fn emit_arguments_length(&mut self) {
        if let Some(actual_argument_count_local) = self.actual_argument_count_local {
            self.push_local_get(actual_argument_count_local);
        } else {
            self.push_i32_const(0);
        }
    }

    pub(in crate::backend::direct_wasm) fn emit_arguments_slot_data_read(
        &mut self,
        slot: &ArgumentsSlot,
    ) {
        if slot.state.mapped {
            if let Some(source_param_local) = slot.source_param_local {
                self.push_local_get(source_param_local);
            } else {
                self.push_local_get(slot.value_local);
            }
        } else {
            self.push_local_get(slot.value_local);
        }
    }

    pub(in crate::backend::direct_wasm) fn emit_arguments_slot_accessor_call(
        &mut self,
        callee: &Expression,
        argument_locals: &[u32],
        argument_count: usize,
        inline_arguments: Option<&[Expression]>,
    ) -> DirectResult<bool> {
        let Some(function_binding) = self.resolve_function_binding_from_expression(callee) else {
            return Ok(false);
        };

        match function_binding {
            LocalFunctionBinding::User(function_name) => {
                let Some(user_function) =
                    self.module.user_function_map.get(&function_name).cloned()
                else {
                    return Ok(false);
                };
                let inline_arguments = inline_arguments
                    .filter(|arguments| arguments.len() == argument_count)
                    .or_else(|| (argument_count == 0).then_some(&[][..]));
                if let Some(inline_arguments) = inline_arguments {
                    if self.can_inline_user_function_call(&user_function, inline_arguments)
                        && self.with_suspended_with_scopes(|compiler| {
                            compiler.emit_inline_user_function_summary_with_arguments(
                                &user_function,
                                inline_arguments,
                            )
                        })?
                    {
                        return Ok(true);
                    }
                }
                if self.with_suspended_with_scopes(|compiler| {
                    compiler.emit_inline_user_function_summary_with_argument_locals(
                        &user_function,
                        argument_locals,
                        argument_count,
                    )
                })? {
                    return Ok(true);
                }
                let visible_param_count = user_function.visible_param_count() as usize;

                for argument_index in 0..visible_param_count {
                    if let Some(argument_local) = argument_locals.get(argument_index).copied() {
                        self.push_local_get(argument_local);
                    } else {
                        self.push_i32_const(JS_UNDEFINED_TAG);
                    }
                }
                self.push_i32_const(argument_count as i32);
                for extra_index in &user_function.extra_argument_indices {
                    if let Some(argument_local) =
                        argument_locals.get(*extra_index as usize).copied()
                    {
                        self.push_local_get(argument_local);
                    } else {
                        self.push_i32_const(JS_UNDEFINED_TAG);
                    }
                }
                self.with_suspended_with_scopes(|compiler| {
                    compiler.push_call(user_function.function_index);
                    let return_value_local = compiler.allocate_temp_local();
                    compiler.push_local_set(return_value_local);
                    compiler.emit_check_global_throw_for_user_call()?;
                    compiler.push_local_get(return_value_local);
                    Ok(())
                })?;
                Ok(true)
            }
            LocalFunctionBinding::Builtin(_) => Ok(false),
        }
    }

    pub(in crate::backend::direct_wasm) fn capture_arguments_slot_value(
        &mut self,
        slot: &ArgumentsSlot,
    ) {
        self.emit_arguments_slot_data_read(slot);
        self.push_local_set(slot.value_local);
    }

    pub(in crate::backend::direct_wasm) fn emit_arguments_slot_read(
        &mut self,
        index: u32,
    ) -> DirectResult<()> {
        self.ensure_arguments_slot(index)?;
        let Some(slot) = self.arguments_slots.get(&index).cloned() else {
            self.push_i32_const(JS_UNDEFINED_TAG);
            return Ok(());
        };

        self.push_local_get(slot.present_local);
        self.instructions.push(0x04);
        self.instructions.push(I32_TYPE);
        self.push_control_frame();

        if slot.state.is_accessor() {
            if let Some(getter) = slot.state.getter.as_ref() {
                if !self.emit_arguments_slot_accessor_call(getter, &[], 0, Some(&[]))? {
                    self.push_i32_const(JS_UNDEFINED_TAG);
                }
            } else {
                self.push_i32_const(JS_UNDEFINED_TAG);
            }
        } else {
            self.emit_arguments_slot_data_read(&slot);
        }

        self.instructions.push(0x05);
        self.push_i32_const(JS_UNDEFINED_TAG);
        self.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_arguments_slot_write_from_local(
        &mut self,
        index: u32,
        value_local: u32,
    ) -> DirectResult<()> {
        self.ensure_arguments_slot(index)?;

        if let Some(slot) = self.arguments_slots.get(&index).cloned() {
            if slot.state.is_accessor() {
                if let Some(setter) = slot.state.setter.as_ref() {
                    if !self.emit_arguments_slot_accessor_call(setter, &[value_local], 1, None)? {
                        self.push_i32_const(JS_UNDEFINED_TAG);
                        self.instructions.push(0x1a);
                    } else {
                        self.instructions.push(0x1a);
                    }
                }
            } else if slot.state.writable {
                if slot.state.mapped {
                    if let Some(source_param_local) = slot.source_param_local {
                        self.push_local_get(value_local);
                        self.push_local_set(source_param_local);
                    } else {
                        self.push_local_get(value_local);
                        self.push_local_set(slot.value_local);
                    }
                } else {
                    self.push_local_get(value_local);
                    self.push_local_set(slot.value_local);
                }
                self.push_i32_const(1);
                self.push_local_set(slot.present_local);
            }

            if !slot.state.is_accessor() && slot.state.mapped {
                self.push_local_get(value_local);
                self.push_local_set(slot.value_local);
            }
        }

        self.push_local_get(value_local);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_arguments_slot_write(
        &mut self,
        index: u32,
        value: &Expression,
    ) -> DirectResult<()> {
        self.ensure_arguments_slot(index)?;
        let temp_local = self.allocate_temp_local();
        self.emit_numeric_expression(value)?;
        self.push_local_set(temp_local);
        self.emit_arguments_slot_write_from_local(index, temp_local)
    }

    pub(in crate::backend::direct_wasm) fn emit_runtime_array_iterator_value_from_local(
        &mut self,
        index_local: u32,
        values: &[Option<Expression>],
    ) -> DirectResult<()> {
        let mut open_frames = 0;
        for (index, value) in values.iter().enumerate() {
            self.push_local_get(index_local);
            self.push_i32_const(index as i32);
            self.push_binary_op(BinaryOp::Equal)?;
            self.instructions.push(0x04);
            self.instructions.push(I32_TYPE);
            self.push_control_frame();
            open_frames += 1;
            if let Some(value) = value {
                self.emit_numeric_expression(value)?;
            } else {
                self.push_i32_const(JS_UNDEFINED_TAG);
            }
            self.instructions.push(0x05);
        }

        self.push_i32_const(JS_UNDEFINED_TAG);
        for _ in 0..open_frames {
            self.instructions.push(0x0b);
            self.pop_control_frame();
        }
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_arguments_slot_delete(&mut self, index: u32) {
        if let Some(slot) = self.arguments_slots.get(&index).cloned() {
            if !slot.state.configurable {
                self.push_i32_const(0);
                return;
            }
            if let Some(entry) = self.arguments_slots.get_mut(&index) {
                entry.state.present = false;
                entry.state.mapped = false;
                entry.state.getter = None;
                entry.state.setter = None;
            }
            self.push_i32_const(0);
            self.push_local_set(slot.present_local);
            if let Some(mapped_local) = slot.mapped_local {
                self.push_i32_const(0);
                self.push_local_set(mapped_local);
            }
        }
        self.push_i32_const(1);
    }

    pub(in crate::backend::direct_wasm) fn is_direct_arguments_object(
        &self,
        expression: &Expression,
    ) -> bool {
        match expression {
            Expression::Identifier(name) if name == "arguments" => self.has_arguments_object(),
            Expression::Identifier(name) => self.direct_arguments_aliases.contains(name),
            _ => false,
        }
    }

    pub(in crate::backend::direct_wasm) fn direct_arguments_callee_expression(
        &self,
    ) -> Option<Expression> {
        self.current_arguments_callee_override.clone().or_else(|| {
            self.current_user_function_name
                .as_ref()
                .map(|name| Expression::Identifier(name.clone()))
        })
    }

    pub(in crate::backend::direct_wasm) fn direct_arguments_has_property(
        &self,
        property_name: &str,
    ) -> bool {
        match property_name {
            "callee" => self.current_arguments_callee_present,
            "length" => self.current_arguments_length_present,
            _ => false,
        }
    }

    pub(in crate::backend::direct_wasm) fn emit_update_arguments_slot_mapping(
        &mut self,
        slot: &ArgumentsSlot,
    ) {
        if let Some(mapped_local) = slot.mapped_local {
            self.push_i32_const(if slot.state.mapped { 1 } else { 0 });
            self.push_local_set(mapped_local);
        }
    }

    pub(in crate::backend::direct_wasm) fn apply_direct_arguments_define_property(
        &mut self,
        index: u32,
        descriptor: &PropertyDescriptorDefinition,
    ) -> DirectResult<bool> {
        self.ensure_arguments_slot(index)?;
        let Some(mut slot) = self.arguments_slots.get(&index).cloned() else {
            return Ok(false);
        };
        let property_exists = slot.state.present;

        if !slot.state.configurable {
            if descriptor.configurable == Some(true) {
                return self.emit_error_throw().map(|_| true);
            }
            if let Some(enumerable) = descriptor.enumerable {
                if enumerable != slot.state.enumerable {
                    return self.emit_error_throw().map(|_| true);
                }
            }
            if descriptor.is_accessor() != slot.state.is_accessor()
                && (descriptor.is_accessor()
                    || descriptor.value.is_some()
                    || descriptor.writable.is_some())
            {
                return self.emit_error_throw().map(|_| true);
            }
            if !slot.state.is_accessor() && !slot.state.writable {
                if descriptor.writable == Some(true) {
                    return self.emit_error_throw().map(|_| true);
                }
                if let Some(value) = descriptor.value.as_ref() {
                    self.capture_arguments_slot_value(&slot);
                    let current_value_local = self.allocate_temp_local();
                    self.push_local_set(current_value_local);
                    self.push_local_get(current_value_local);
                    self.emit_numeric_expression(value)?;
                    self.push_binary_op(BinaryOp::NotEqual)?;
                    self.instructions.push(0x04);
                    self.instructions.push(EMPTY_BLOCK_TYPE);
                    self.push_control_frame();
                    self.emit_error_throw()?;
                    self.instructions.push(0x0b);
                    self.pop_control_frame();
                }
            }
        }

        if descriptor.is_accessor() {
            slot.state.present = true;
            slot.state.mapped = false;
            slot.state.writable = false;
            slot.state.enumerable = descriptor.enumerable.unwrap_or(if property_exists {
                slot.state.enumerable
            } else {
                false
            });
            slot.state.configurable = descriptor.configurable.unwrap_or(if property_exists {
                slot.state.configurable
            } else {
                false
            });
            slot.state.getter = descriptor.getter.clone();
            slot.state.setter = descriptor.setter.clone();
            if let Some(mapped_local) = slot.mapped_local {
                self.push_i32_const(0);
                self.push_local_set(mapped_local);
            }
            self.push_i32_const(1);
            self.push_local_set(slot.present_local);
            self.arguments_slots.insert(index, slot);
            return Ok(true);
        }

        if slot.state.is_accessor() {
            slot.state.getter = None;
            slot.state.setter = None;
        }

        if let Some(value) = descriptor.value.as_ref() {
            let temp_local = self.allocate_temp_local();
            self.emit_numeric_expression(value)?;
            self.push_local_set(temp_local);
            if slot.state.mapped {
                if let Some(source_param_local) = slot.source_param_local {
                    self.push_local_get(temp_local);
                    self.push_local_set(source_param_local);
                }
            }
            self.push_local_get(temp_local);
            self.push_local_set(slot.value_local);
        } else if descriptor.writable == Some(false) && slot.state.mapped {
            self.capture_arguments_slot_value(&slot);
        }

        slot.state.present = true;
        slot.state.writable = descriptor.writable.unwrap_or(if property_exists {
            slot.state.writable
        } else {
            false
        });
        slot.state.enumerable = descriptor.enumerable.unwrap_or(if property_exists {
            slot.state.enumerable
        } else {
            false
        });
        slot.state.configurable = descriptor.configurable.unwrap_or(if property_exists {
            slot.state.configurable
        } else {
            false
        });

        if descriptor.writable == Some(false) {
            slot.state.mapped = false;
        }

        self.push_i32_const(1);
        self.push_local_set(slot.present_local);
        self.emit_update_arguments_slot_mapping(&slot);
        self.arguments_slots.insert(index, slot);
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_direct_arguments_length(
        &mut self,
    ) -> DirectResult<()> {
        if !self.current_arguments_length_present {
            self.push_i32_const(JS_UNDEFINED_TAG);
            return Ok(());
        }
        if let Some(value) = self.current_arguments_length_override.clone() {
            self.emit_numeric_expression(&value)?;
            return Ok(());
        }
        self.emit_arguments_length();
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_direct_arguments_callee(
        &mut self,
    ) -> DirectResult<()> {
        if self.strict_mode {
            return self.emit_error_throw();
        }
        if !self.current_arguments_callee_present {
            self.push_i32_const(JS_UNDEFINED_TAG);
            return Ok(());
        }
        if let Some(value) = self.direct_arguments_callee_expression() {
            self.emit_numeric_expression(&value)?;
        } else {
            self.push_i32_const(JS_UNDEFINED_TAG);
        }
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_property_name_match(
        &mut self,
        property_local: u32,
        property_name: &str,
    ) -> DirectResult<()> {
        let (ptr, _) = self.module.intern_string(property_name.as_bytes().to_vec());
        self.push_local_get(property_local);
        self.push_i32_const(ptr as i32);
        self.push_binary_op(BinaryOp::Equal)
    }

    pub(in crate::backend::direct_wasm) fn emit_dynamic_arguments_binding_property_read(
        &mut self,
        binding: &ArgumentsValueBinding,
        property: &Expression,
    ) -> DirectResult<()> {
        let property_local = self.allocate_temp_local();
        self.emit_numeric_expression(property)?;
        self.push_local_set(property_local);

        let mut open_frames = 0;

        self.emit_property_name_match(property_local, "length")?;
        self.instructions.push(0x04);
        self.instructions.push(I32_TYPE);
        self.push_control_frame();
        open_frames += 1;
        if binding.length_present {
            self.emit_numeric_expression(&binding.length_value)?;
        } else {
            self.push_i32_const(JS_UNDEFINED_TAG);
        }
        self.instructions.push(0x05);

        self.emit_property_name_match(property_local, "callee")?;
        self.instructions.push(0x04);
        self.instructions.push(I32_TYPE);
        self.push_control_frame();
        open_frames += 1;
        if binding.strict {
            self.emit_error_throw()?;
        } else if binding.callee_present {
            if let Some(value) = binding.callee_value.as_ref() {
                self.emit_numeric_expression(value)?;
            } else {
                self.push_i32_const(JS_UNDEFINED_TAG);
            }
        } else {
            self.push_i32_const(JS_UNDEFINED_TAG);
        }
        self.instructions.push(0x05);

        self.emit_property_name_match(property_local, "constructor")?;
        self.instructions.push(0x04);
        self.instructions.push(I32_TYPE);
        self.push_control_frame();
        open_frames += 1;
        self.push_i32_const(JS_TYPEOF_FUNCTION_TAG);
        self.instructions.push(0x05);

        for (index, value) in binding.values.iter().enumerate() {
            self.push_local_get(property_local);
            self.push_i32_const(index as i32);
            self.push_binary_op(BinaryOp::Equal)?;
            self.instructions.push(0x04);
            self.instructions.push(I32_TYPE);
            self.push_control_frame();
            open_frames += 1;
            self.emit_numeric_expression(value)?;
            self.instructions.push(0x05);
        }

        self.push_i32_const(JS_UNDEFINED_TAG);
        for _ in 0..open_frames {
            self.instructions.push(0x0b);
            self.pop_control_frame();
        }
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_dynamic_direct_arguments_property_read_from_local(
        &mut self,
        property_local: u32,
    ) -> DirectResult<()> {
        let mut open_frames = 0;

        self.emit_property_name_match(property_local, "length")?;
        self.instructions.push(0x04);
        self.instructions.push(I32_TYPE);
        self.push_control_frame();
        open_frames += 1;
        self.emit_direct_arguments_length()?;
        self.instructions.push(0x05);

        self.emit_property_name_match(property_local, "callee")?;
        self.instructions.push(0x04);
        self.instructions.push(I32_TYPE);
        self.push_control_frame();
        open_frames += 1;
        self.emit_direct_arguments_callee()?;
        self.instructions.push(0x05);

        self.emit_property_name_match(property_local, "constructor")?;
        self.instructions.push(0x04);
        self.instructions.push(I32_TYPE);
        self.push_control_frame();
        open_frames += 1;
        self.push_i32_const(JS_TYPEOF_FUNCTION_TAG);
        self.instructions.push(0x05);

        let mut indices = self.arguments_slots.keys().copied().collect::<Vec<_>>();
        indices.sort_unstable();
        for index in indices {
            self.push_local_get(property_local);
            self.push_i32_const(index as i32);
            self.push_binary_op(BinaryOp::Equal)?;
            self.instructions.push(0x04);
            self.instructions.push(I32_TYPE);
            self.push_control_frame();
            open_frames += 1;
            self.emit_arguments_slot_read(index)?;
            self.instructions.push(0x05);
        }

        self.push_i32_const(JS_UNDEFINED_TAG);
        for _ in 0..open_frames {
            self.instructions.push(0x0b);
            self.pop_control_frame();
        }
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_dynamic_direct_arguments_property_read(
        &mut self,
        property: &Expression,
    ) -> DirectResult<()> {
        let property_local = self.allocate_temp_local();
        self.emit_numeric_expression(property)?;
        self.push_local_set(property_local);
        self.emit_dynamic_direct_arguments_property_read_from_local(property_local)
    }

    pub(in crate::backend::direct_wasm) fn emit_dynamic_direct_arguments_property_write(
        &mut self,
        property: &Expression,
        value: &Expression,
    ) -> DirectResult<()> {
        let property_local = self.allocate_temp_local();
        self.emit_numeric_expression(property)?;
        self.push_local_set(property_local);

        let specialized_rhs = match value {
            Expression::Binary { op, left, right }
                if *op == BinaryOp::Multiply
                    && matches!(
                        left.as_ref(),
                        Expression::Member {
                            object,
                            property: left_property,
                        } if self.is_direct_arguments_object(object)
                            && **left_property == *property
                    ) =>
            {
                let rhs_local = self.allocate_temp_local();
                self.emit_numeric_expression(right)?;
                self.push_local_set(rhs_local);
                Some((*op, rhs_local))
            }
            _ => None,
        };

        let value_local = self.allocate_temp_local();
        if specialized_rhs.is_none() {
            self.emit_numeric_expression(value)?;
            self.push_local_set(value_local);
        }

        let mut open_frames = 0;
        let mut indices = self.arguments_slots.keys().copied().collect::<Vec<_>>();
        indices.sort_unstable();
        for index in indices {
            self.push_local_get(property_local);
            self.push_i32_const(index as i32);
            self.push_binary_op(BinaryOp::Equal)?;
            self.instructions.push(0x04);
            self.instructions.push(I32_TYPE);
            self.push_control_frame();
            open_frames += 1;
            if let Some((BinaryOp::Multiply, rhs_local)) = specialized_rhs {
                let result_local = self.allocate_temp_local();
                let slot = self
                    .arguments_slots
                    .get(&index)
                    .cloned()
                    .expect("tracked argument slot should exist");
                self.push_local_get(slot.present_local);
                self.instructions.push(0x04);
                self.instructions.push(I32_TYPE);
                self.push_control_frame();
                self.emit_arguments_slot_read(index)?;
                self.push_local_get(rhs_local);
                self.push_binary_op(BinaryOp::Multiply)?;
                self.instructions.push(0x05);
                self.push_i32_const(JS_NAN_TAG);
                self.instructions.push(0x0b);
                self.pop_control_frame();
                self.push_local_set(result_local);
                self.emit_arguments_slot_write_from_local(index, result_local)?;
            } else {
                self.emit_arguments_slot_write_from_local(index, value_local)?;
            }
            self.instructions.push(0x05);
        }

        if specialized_rhs.is_some() {
            self.push_i32_const(JS_NAN_TAG);
        } else {
            self.push_local_get(value_local);
        }
        for _ in 0..open_frames {
            self.instructions.push(0x0b);
            self.pop_control_frame();
        }
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn apply_current_arguments_effect(
        &mut self,
        property_name: &str,
        effect: ArgumentsPropertyEffect,
    ) {
        match property_name {
            "callee" => {
                if self.strict_mode {
                    return;
                }
                match effect {
                    ArgumentsPropertyEffect::Assign(value) => {
                        self.current_arguments_callee_present = true;
                        self.current_arguments_callee_override = Some(value);
                    }
                    ArgumentsPropertyEffect::Delete => {
                        self.current_arguments_callee_present = false;
                        self.current_arguments_callee_override = None;
                    }
                }
            }
            "length" => match effect {
                ArgumentsPropertyEffect::Assign(value) => {
                    self.current_arguments_length_present = true;
                    self.current_arguments_length_override = Some(value);
                }
                ArgumentsPropertyEffect::Delete => {
                    self.current_arguments_length_present = false;
                    self.current_arguments_length_override = None;
                }
            },
            _ => {}
        }
    }

    pub(in crate::backend::direct_wasm) fn update_named_arguments_binding_effect(
        &mut self,
        object: &Expression,
        property_name: &str,
        effect: ArgumentsPropertyEffect,
    ) -> bool {
        let Expression::Identifier(name) = object else {
            return false;
        };
        if let Some(binding) = self.local_arguments_bindings.get_mut(name) {
            binding.apply_named_effect(property_name, effect.clone());
            return true;
        }
        if let Some(binding) = self.module.global_arguments_bindings.get_mut(name) {
            binding.apply_named_effect(property_name, effect);
            return true;
        }
        false
    }

    pub(in crate::backend::direct_wasm) fn resolve_arguments_callee_strictness(
        &self,
        expression: &Expression,
    ) -> Option<bool> {
        let Expression::Member { object, property } = expression else {
            return None;
        };
        if !matches!(property.as_ref(), Expression::String(property_name) if property_name == "callee")
        {
            return None;
        }
        if self.is_direct_arguments_object(object) {
            return Some(self.strict_mode);
        }
        self.resolve_arguments_binding_from_expression(object)
            .map(|binding| binding.strict)
    }

    pub(in crate::backend::direct_wasm) fn resolve_arguments_binding_from_expression(
        &self,
        expression: &Expression,
    ) -> Option<ArgumentsValueBinding> {
        match expression {
            Expression::Identifier(name) => self
                .local_arguments_bindings
                .get(name)
                .cloned()
                .or_else(|| self.module.global_arguments_bindings.get(name).cloned()),
            Expression::Call { callee, arguments } | Expression::New { callee, arguments } => {
                let Expression::Identifier(name) = callee.as_ref() else {
                    return None;
                };
                let user_function = self.resolve_user_function_from_callee_name(name)?;
                if !user_function.returns_arguments_object {
                    return None;
                }
                Some(ArgumentsValueBinding::for_user_function(
                    user_function,
                    self.expand_call_arguments(arguments),
                ))
            }
            _ => None,
        }
    }
}
