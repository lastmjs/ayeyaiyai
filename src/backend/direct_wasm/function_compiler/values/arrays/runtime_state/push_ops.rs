use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_runtime_array_push_from_local(
        &mut self,
        name: &str,
        value_local: u32,
        value_expression: &Expression,
    ) -> DirectResult<bool> {
        let binding_name = self
            .resolve_runtime_array_binding_name(name)
            .unwrap_or_else(|| name.to_string());
        let Some(length_local) = self
            .state
            .speculation
            .static_semantics
            .runtime_array_length_local(&binding_name)
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
            self.state.emission.output.instructions.push(0x04);
            self.state.emission.output.instructions.push(I32_TYPE);
            self.push_control_frame();
            self.push_local_get(length_local);
            self.state.emission.output.instructions.push(0x05);
            self.emit_runtime_array_push_with_length_local(
                &binding_name,
                length_local,
                value_local,
                value_expression,
            )?;
            self.state.emission.output.instructions.push(0x0b);
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
            self.state.emission.output.instructions.push(0x04);
            self.state
                .emission
                .output
                .instructions
                .push(EMPTY_BLOCK_TYPE);
            self.push_control_frame();
            open_frames += 1;
            self.update_tracked_array_specialized_function_value(name, index, value_expression)?;
            if self.emit_runtime_array_slot_write_from_local(name, index, value_local)? {
                self.state.emission.output.instructions.push(0x1a);
            }
            self.push_i32_const(1);
            self.push_local_set(matched_local);
            self.state.emission.output.instructions.push(0x05);
        }
        self.push_local_get(matched_local);
        self.state.emission.output.instructions.push(0x04);
        self.state
            .emission
            .output
            .instructions
            .push(EMPTY_BLOCK_TYPE);
        self.push_control_frame();
        self.state.emission.output.instructions.push(0x05);
        self.push_local_get(length_local);
        self.push_i32_const(1);
        self.push_binary_op(BinaryOp::Add)?;
        self.push_local_set(length_local);
        self.state.emission.output.instructions.push(0x0b);
        self.pop_control_frame();
        for _ in 0..open_frames {
            self.state.emission.output.instructions.push(0x0b);
            self.pop_control_frame();
        }
        self.push_local_get(length_local);
        Ok(true)
    }
}
