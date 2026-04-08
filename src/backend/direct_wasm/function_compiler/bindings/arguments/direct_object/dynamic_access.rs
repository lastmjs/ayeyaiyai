use super::*;

impl<'a> FunctionCompiler<'a> {
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
        self.state.emission.output.instructions.push(0x04);
        self.state.emission.output.instructions.push(I32_TYPE);
        self.push_control_frame();
        open_frames += 1;
        if binding.length_present {
            self.emit_numeric_expression(&binding.length_value)?;
        } else {
            self.push_i32_const(JS_UNDEFINED_TAG);
        }
        self.state.emission.output.instructions.push(0x05);

        self.emit_property_name_match(property_local, "callee")?;
        self.state.emission.output.instructions.push(0x04);
        self.state.emission.output.instructions.push(I32_TYPE);
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
        self.state.emission.output.instructions.push(0x05);

        self.emit_property_name_match(property_local, "constructor")?;
        self.state.emission.output.instructions.push(0x04);
        self.state.emission.output.instructions.push(I32_TYPE);
        self.push_control_frame();
        open_frames += 1;
        self.push_i32_const(JS_TYPEOF_FUNCTION_TAG);
        self.state.emission.output.instructions.push(0x05);

        for (index, value) in binding.values.iter().enumerate() {
            self.push_local_get(property_local);
            self.push_i32_const(index as i32);
            self.push_binary_op(BinaryOp::Equal)?;
            self.state.emission.output.instructions.push(0x04);
            self.state.emission.output.instructions.push(I32_TYPE);
            self.push_control_frame();
            open_frames += 1;
            self.emit_numeric_expression(value)?;
            self.state.emission.output.instructions.push(0x05);
        }

        self.push_i32_const(JS_UNDEFINED_TAG);
        for _ in 0..open_frames {
            self.state.emission.output.instructions.push(0x0b);
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
        self.state.emission.output.instructions.push(0x04);
        self.state.emission.output.instructions.push(I32_TYPE);
        self.push_control_frame();
        open_frames += 1;
        self.emit_direct_arguments_length()?;
        self.state.emission.output.instructions.push(0x05);

        self.emit_property_name_match(property_local, "callee")?;
        self.state.emission.output.instructions.push(0x04);
        self.state.emission.output.instructions.push(I32_TYPE);
        self.push_control_frame();
        open_frames += 1;
        self.emit_direct_arguments_callee()?;
        self.state.emission.output.instructions.push(0x05);

        self.emit_property_name_match(property_local, "constructor")?;
        self.state.emission.output.instructions.push(0x04);
        self.state.emission.output.instructions.push(I32_TYPE);
        self.push_control_frame();
        open_frames += 1;
        self.push_i32_const(JS_TYPEOF_FUNCTION_TAG);
        self.state.emission.output.instructions.push(0x05);

        let mut indices = self
            .state
            .parameters
            .arguments_slots
            .keys()
            .copied()
            .collect::<Vec<_>>();
        indices.sort_unstable();
        for index in indices {
            self.push_local_get(property_local);
            self.push_i32_const(index as i32);
            self.push_binary_op(BinaryOp::Equal)?;
            self.state.emission.output.instructions.push(0x04);
            self.state.emission.output.instructions.push(I32_TYPE);
            self.push_control_frame();
            open_frames += 1;
            self.emit_arguments_slot_read(index)?;
            self.state.emission.output.instructions.push(0x05);
        }

        self.push_i32_const(JS_UNDEFINED_TAG);
        for _ in 0..open_frames {
            self.state.emission.output.instructions.push(0x0b);
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
        let mut indices = self
            .state
            .parameters
            .arguments_slots
            .keys()
            .copied()
            .collect::<Vec<_>>();
        indices.sort_unstable();
        for index in indices {
            self.push_local_get(property_local);
            self.push_i32_const(index as i32);
            self.push_binary_op(BinaryOp::Equal)?;
            self.state.emission.output.instructions.push(0x04);
            self.state.emission.output.instructions.push(I32_TYPE);
            self.push_control_frame();
            open_frames += 1;
            if let Some((BinaryOp::Multiply, rhs_local)) = specialized_rhs {
                let result_local = self.allocate_temp_local();
                let slot = self
                    .state
                    .parameters
                    .arguments_slots
                    .get(&index)
                    .cloned()
                    .expect("tracked argument slot should exist");
                self.push_local_get(slot.present_local);
                self.state.emission.output.instructions.push(0x04);
                self.state.emission.output.instructions.push(I32_TYPE);
                self.push_control_frame();
                self.emit_arguments_slot_read(index)?;
                self.push_local_get(rhs_local);
                self.push_binary_op(BinaryOp::Multiply)?;
                self.state.emission.output.instructions.push(0x05);
                self.push_i32_const(JS_NAN_TAG);
                self.state.emission.output.instructions.push(0x0b);
                self.pop_control_frame();
                self.push_local_set(result_local);
                self.emit_arguments_slot_write_from_local(index, result_local)?;
            } else {
                self.emit_arguments_slot_write_from_local(index, value_local)?;
            }
            self.state.emission.output.instructions.push(0x05);
        }

        if specialized_rhs.is_some() {
            self.push_i32_const(JS_NAN_TAG);
        } else {
            self.push_local_get(value_local);
        }
        for _ in 0..open_frames {
            self.state.emission.output.instructions.push(0x0b);
            self.pop_control_frame();
        }
        Ok(())
    }
}
