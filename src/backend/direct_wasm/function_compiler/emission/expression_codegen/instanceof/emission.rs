use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_instanceof_expression(
        &mut self,
        left: &Expression,
        right: &Expression,
    ) -> DirectResult<()> {
        let has_instance_property = Expression::Member {
            object: Box::new(Expression::Identifier("Symbol".to_string())),
            property: Box::new(Expression::String("hasInstance".to_string())),
        };
        if let Some(function_binding) =
            self.resolve_member_function_binding(right, &has_instance_property)
        {
            let left_local = self.allocate_temp_local();
            self.emit_numeric_expression(left)?;
            self.push_local_set(left_local);
            let result_local = self.allocate_temp_local();
            match function_binding {
                LocalFunctionBinding::User(function_name) => {
                    let Some(user_function) = self.user_function(&function_name).cloned() else {
                        self.push_i32_const(0);
                        return Ok(());
                    };
                    let argument_locals = [left_local];
                    if let Some(capture_slots) =
                        self.resolve_member_function_capture_slots(right, &has_instance_property)
                    {
                        self.emit_user_function_call_with_new_target_and_this_expression_and_bound_captures_from_argument_locals(
                            &user_function,
                            &argument_locals,
                            1,
                            JS_UNDEFINED_TAG,
                            right,
                            &capture_slots,
                        )?;
                    } else {
                        self.emit_user_function_call_with_new_target_and_this_expression_from_argument_locals(
                            &user_function,
                            &argument_locals,
                            1,
                            JS_UNDEFINED_TAG,
                            right,
                        )?;
                    }
                    self.push_local_set(result_local);
                    self.emit_instanceof_truthy_from_local(result_local)?;
                    return Ok(());
                }
                LocalFunctionBinding::Builtin(_) => {
                    self.emit_numeric_expression(right)?;
                    self.state.emission.output.instructions.push(0x1a);
                    self.push_i32_const(0);
                    return Ok(());
                }
            }
        }

        let materialized_right = self.materialize_static_expression(right);
        if self.expression_is_builtin_array_constructor(&materialized_right) {
            self.emit_numeric_expression(left)?;
            self.state.emission.output.instructions.push(0x1a);
            self.emit_numeric_expression(right)?;
            self.state.emission.output.instructions.push(0x1a);
            self.push_i32_const(if self.expression_is_known_array_value(left) {
                1
            } else {
                0
            });
            return Ok(());
        }

        if let Expression::Identifier(name) = &materialized_right {
            if let Some(expected_values) = native_error_instanceof_values(name) {
                let left_local = self.allocate_temp_local();
                self.emit_numeric_expression(left)?;
                self.push_local_set(left_local);
                self.emit_numeric_expression(right)?;
                self.state.emission.output.instructions.push(0x1a);
                if expected_values.len() == 1 {
                    let expected_value = expected_values[0];
                    self.push_local_get(left_local);
                    self.push_i32_const(expected_value);
                    self.push_binary_op(BinaryOp::Equal)?;
                    return Ok(());
                }

                let matched_local = self.allocate_temp_local();
                self.push_i32_const(0);
                self.push_local_set(matched_local);
                for expected_value in expected_values {
                    self.push_local_get(left_local);
                    self.push_i32_const(expected_value);
                    self.push_binary_op(BinaryOp::Equal)?;
                    self.state.emission.output.instructions.push(0x04);
                    self.state
                        .emission
                        .output
                        .instructions
                        .push(EMPTY_BLOCK_TYPE);
                    self.push_control_frame();
                    self.push_i32_const(1);
                    self.push_local_set(matched_local);
                    self.state.emission.output.instructions.push(0x0b);
                    self.pop_control_frame();
                }
                self.push_local_get(matched_local);
                return Ok(());
            }
        }

        if let Some(prototype_expression) =
            self.resolve_instanceof_prototype_expression(&materialized_right)
        {
            let left_local = self.allocate_temp_local();
            self.emit_numeric_expression(left)?;
            self.push_local_set(left_local);
            let static_result = if self.expression_is_known_non_object_value_for_instanceof(left) {
                false
            } else {
                self.expression_inherits_from_prototype_for_instanceof(left, &prototype_expression)
            };
            if let Some(getter_binding) = self.resolve_member_getter_binding(
                &materialized_right,
                &Expression::String("prototype".to_string()),
            ) {
                match getter_binding {
                    LocalFunctionBinding::User(function_name) => {
                        if let Some(user_function) = self.user_function(&function_name).cloned() {
                            self.emit_user_function_call_with_new_target_and_this_expression_from_argument_locals(
                                &user_function,
                                &[],
                                0,
                                JS_UNDEFINED_TAG,
                                &materialized_right,
                            )?;
                            self.state.emission.output.instructions.push(0x1a);
                        } else {
                            self.emit_numeric_expression(&materialized_right)?;
                            self.state.emission.output.instructions.push(0x1a);
                        }
                    }
                    LocalFunctionBinding::Builtin(function_name) => {
                        let getter_callee = Expression::Identifier(function_name);
                        if !self.emit_arguments_slot_accessor_call(
                            &getter_callee,
                            &[],
                            0,
                            Some(&[]),
                        )? {
                            self.push_i32_const(JS_UNDEFINED_TAG);
                        }
                        self.state.emission.output.instructions.push(0x1a);
                    }
                }
            } else {
                self.emit_numeric_expression(right)?;
                self.state.emission.output.instructions.push(0x1a);
            }
            self.push_i32_const(if static_result { 1 } else { 0 });
            return Ok(());
        }

        self.emit_numeric_expression(left)?;
        self.state.emission.output.instructions.push(0x1a);
        self.emit_numeric_expression(right)?;
        self.state.emission.output.instructions.push(0x1a);
        self.push_i32_const(0);
        Ok(())
    }
}
