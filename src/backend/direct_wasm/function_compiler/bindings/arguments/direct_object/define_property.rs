use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn apply_direct_arguments_define_property(
        &mut self,
        index: u32,
        descriptor: &PropertyDescriptorDefinition,
    ) -> DirectResult<bool> {
        self.ensure_arguments_slot(index)?;
        let Some(mut slot) = self.state.parameters.arguments_slots.get(&index).cloned() else {
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
                    self.state.emission.output.instructions.push(0x04);
                    self.state
                        .emission
                        .output
                        .instructions
                        .push(EMPTY_BLOCK_TYPE);
                    self.push_control_frame();
                    self.emit_error_throw()?;
                    self.state.emission.output.instructions.push(0x0b);
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
            self.state.parameters.arguments_slots.insert(index, slot);
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
        self.state.parameters.arguments_slots.insert(index, slot);
        Ok(true)
    }
}
