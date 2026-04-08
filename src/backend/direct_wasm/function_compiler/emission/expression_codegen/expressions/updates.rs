use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn emit_update_expression(
        &mut self,
        name: &str,
        op: UpdateOp,
        prefix: bool,
    ) -> DirectResult<()> {
        if let Some(scope_object) = self.resolve_with_scope_binding(name)? {
            self.emit_scoped_property_update(&scope_object, name, op, prefix)?;
            return Ok(());
        }

        let opcode = match op {
            UpdateOp::Increment => 0x6a,
            UpdateOp::Decrement => 0x6b,
        };

        let previous_kind = self
            .lookup_identifier_kind(name)
            .unwrap_or(StaticValueKind::Unknown);

        match previous_kind {
            StaticValueKind::Undefined
            | StaticValueKind::String
            | StaticValueKind::Object
            | StaticValueKind::Function
            | StaticValueKind::Symbol
            | StaticValueKind::BigInt => {
                let nan_local = self.allocate_temp_local();
                self.push_i32_const(JS_NAN_TAG);
                self.push_local_set(nan_local);
                self.emit_store_identifier_from_local(name, nan_local)?;
                self.note_identifier_numeric_kind(name);
                self.push_local_get(nan_local);
                return Ok(());
            }
            StaticValueKind::Null => {
                let previous_local = self.allocate_temp_local();
                let next_local = self.allocate_temp_local();
                self.push_i32_const(0);
                self.push_local_set(previous_local);
                self.push_i32_const(match op {
                    UpdateOp::Increment => 1,
                    UpdateOp::Decrement => -1,
                });
                self.push_local_set(next_local);
                self.emit_store_identifier_from_local(name, next_local)?;
                self.note_identifier_numeric_kind(name);
                if prefix {
                    self.push_local_get(next_local);
                } else {
                    self.push_local_get(previous_local);
                }
                return Ok(());
            }
            _ => {}
        }

        if let Some((_, local_index)) = self.resolve_current_local_binding(name) {
            if prefix {
                self.push_local_get(local_index);
                self.push_i32_const(1);
                self.state.emission.output.instructions.push(opcode);
                self.push_local_tee(local_index);
            } else {
                self.push_local_get(local_index);
                self.push_local_get(local_index);
                self.push_i32_const(1);
                self.state.emission.output.instructions.push(opcode);
                self.push_local_set(local_index);
            }
        } else if let Some(global_index) = self
            .backend
            .global_semantics
            .names
            .bindings
            .get(name)
            .copied()
        {
            if prefix {
                let result_local = self.allocate_temp_local();
                self.push_global_get(global_index);
                self.push_i32_const(1);
                self.state.emission.output.instructions.push(opcode);
                self.push_local_tee(result_local);
                self.push_global_set(global_index);
                self.push_local_get(result_local);
            } else {
                let previous_local = self.allocate_temp_local();
                self.push_global_get(global_index);
                self.push_local_tee(previous_local);
                self.push_i32_const(1);
                self.state.emission.output.instructions.push(opcode);
                self.push_global_set(global_index);
                self.push_local_get(previous_local);
            }
        } else if let Some(hidden_name) = self.resolve_user_function_capture_hidden_name(name)
            && let Some(binding) = self.backend.implicit_global_binding(&hidden_name)
        {
            let previous_local = self.allocate_temp_local();
            let next_local = self.allocate_temp_local();
            self.push_global_get(binding.present_index);
            self.state.emission.output.instructions.push(0x04);
            self.state
                .emission
                .output
                .instructions
                .push(EMPTY_BLOCK_TYPE);
            self.push_control_frame();
            self.push_global_get(binding.value_index);
            self.push_local_tee(previous_local);
            self.push_i32_const(1);
            self.state.emission.output.instructions.push(opcode);
            self.push_local_tee(next_local);
            self.push_global_set(binding.value_index);
            self.state.emission.output.instructions.push(0x05);
            self.emit_named_error_throw("ReferenceError")?;
            self.state.emission.output.instructions.push(0x0b);
            self.pop_control_frame();
            if prefix {
                self.push_local_get(next_local);
            } else {
                self.push_local_get(previous_local);
            }
        } else if let Some(binding) = self.backend.implicit_global_binding(name) {
            let previous_local = self.allocate_temp_local();
            let next_local = self.allocate_temp_local();
            self.push_global_get(binding.present_index);
            self.state.emission.output.instructions.push(0x04);
            self.state
                .emission
                .output
                .instructions
                .push(EMPTY_BLOCK_TYPE);
            self.push_control_frame();
            self.push_global_get(binding.value_index);
            self.push_local_tee(previous_local);
            self.push_i32_const(1);
            self.state.emission.output.instructions.push(opcode);
            self.push_local_tee(next_local);
            self.push_global_set(binding.value_index);
            self.state.emission.output.instructions.push(0x05);
            self.emit_named_error_throw("ReferenceError")?;
            self.state.emission.output.instructions.push(0x0b);
            self.pop_control_frame();
            if prefix {
                self.push_local_get(next_local);
            } else {
                self.push_local_get(previous_local);
            }
        } else {
            self.emit_named_error_throw("ReferenceError")?;
        }
        self.note_identifier_numeric_kind(name);
        Ok(())
    }
}
