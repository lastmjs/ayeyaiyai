use super::*;

impl<'a> FunctionCompiler<'a> {
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
        let Some(slot) = self.state.parameters.arguments_slots.get(&index).cloned() else {
            self.push_i32_const(JS_UNDEFINED_TAG);
            return Ok(());
        };

        self.push_local_get(slot.present_local);
        self.state.emission.output.instructions.push(0x04);
        self.state.emission.output.instructions.push(I32_TYPE);
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

        self.state.emission.output.instructions.push(0x05);
        self.push_i32_const(JS_UNDEFINED_TAG);
        self.state.emission.output.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_arguments_slot_write_from_local(
        &mut self,
        index: u32,
        value_local: u32,
    ) -> DirectResult<()> {
        self.ensure_arguments_slot(index)?;

        if let Some(slot) = self.state.parameters.arguments_slots.get(&index).cloned() {
            if slot.state.is_accessor() {
                if let Some(setter) = slot.state.setter.as_ref() {
                    if !self.emit_arguments_slot_accessor_call(setter, &[value_local], 1, None)? {
                        self.push_i32_const(JS_UNDEFINED_TAG);
                        self.state.emission.output.instructions.push(0x1a);
                    } else {
                        self.state.emission.output.instructions.push(0x1a);
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

    pub(in crate::backend::direct_wasm) fn emit_arguments_slot_delete(&mut self, index: u32) {
        if let Some(slot) = self.state.parameters.arguments_slots.get(&index).cloned() {
            if !slot.state.configurable {
                self.push_i32_const(0);
                return;
            }
            if let Some(entry) = self.state.parameters.arguments_slots.get_mut(&index) {
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
}
