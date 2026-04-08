use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_assert_compare_array_call(
        &mut self,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        let [
            CallArgument::Expression(actual),
            CallArgument::Expression(expected),
            rest @ ..,
        ] = arguments
        else {
            return Ok(false);
        };
        let Some(expected_binding) = self.resolve_array_binding_from_expression(expected) else {
            return Ok(false);
        };

        self.emit_numeric_expression(actual)?;
        self.state.emission.output.instructions.push(0x1a);
        self.emit_numeric_expression(expected)?;
        self.state.emission.output.instructions.push(0x1a);
        for argument in rest {
            match argument {
                CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                    self.emit_numeric_expression(expression)?;
                    self.state.emission.output.instructions.push(0x1a);
                }
            }
        }

        if self.has_current_user_function()
            && matches!(
                actual,
                Expression::Identifier(_) | Expression::Member { .. }
            )
        {
            return Ok(false);
        }

        if matches!(
            actual,
            Expression::Identifier(_) | Expression::Member { .. }
        ) {
            return self
                .emit_runtime_assert_compare_array_against_expected(actual, &expected_binding);
        }

        let Some(actual_binding) = self.resolve_array_binding_from_expression(actual) else {
            return Ok(false);
        };
        if !self.array_bindings_equal(&actual_binding, &expected_binding) {
            self.emit_error_throw()?;
        } else {
            self.push_i32_const(JS_UNDEFINED_TAG);
        }
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_runtime_assert_compare_array_against_expected(
        &mut self,
        actual: &Expression,
        expected_binding: &ArrayValueBinding,
    ) -> DirectResult<bool> {
        let mismatch_local = self.allocate_temp_local();
        self.push_i32_const(0);
        self.push_local_set(mismatch_local);

        self.emit_numeric_expression(&Expression::Member {
            object: Box::new(actual.clone()),
            property: Box::new(Expression::String("length".to_string())),
        })?;
        self.push_i32_const(expected_binding.values.len() as i32);
        self.push_binary_op(BinaryOp::NotEqual)?;
        self.state.emission.output.instructions.push(0x04);
        self.state
            .emission
            .output
            .instructions
            .push(EMPTY_BLOCK_TYPE);
        self.push_control_frame();
        self.push_i32_const(1);
        self.push_local_set(mismatch_local);
        self.state.emission.output.instructions.push(0x0b);
        self.pop_control_frame();

        for (index, expected_value) in expected_binding.values.iter().enumerate() {
            self.emit_numeric_expression(&Expression::Member {
                object: Box::new(actual.clone()),
                property: Box::new(Expression::Number(index as f64)),
            })?;
            self.emit_numeric_expression(&expected_value.clone().unwrap_or(Expression::Undefined))?;
            self.push_binary_op(BinaryOp::NotEqual)?;
            self.state.emission.output.instructions.push(0x04);
            self.state
                .emission
                .output
                .instructions
                .push(EMPTY_BLOCK_TYPE);
            self.push_control_frame();
            self.push_i32_const(1);
            self.push_local_set(mismatch_local);
            self.state.emission.output.instructions.push(0x0b);
            self.pop_control_frame();
        }

        self.push_local_get(mismatch_local);
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
        self.push_i32_const(JS_UNDEFINED_TAG);
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_compare_array_call(
        &mut self,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        let [
            CallArgument::Expression(actual),
            CallArgument::Expression(expected),
            rest @ ..,
        ] = arguments
        else {
            return Ok(false);
        };
        let Some(expected_binding) = self.resolve_array_binding_from_expression(expected) else {
            return Ok(false);
        };

        self.emit_numeric_expression(actual)?;
        self.state.emission.output.instructions.push(0x1a);
        self.emit_numeric_expression(expected)?;
        self.state.emission.output.instructions.push(0x1a);
        for argument in rest {
            match argument {
                CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                    self.emit_numeric_expression(expression)?;
                    self.state.emission.output.instructions.push(0x1a);
                }
            }
        }

        if self.has_current_user_function()
            && matches!(
                actual,
                Expression::Identifier(_) | Expression::Member { .. }
            )
        {
            return Ok(false);
        }

        if matches!(
            actual,
            Expression::Identifier(_) | Expression::Member { .. }
        ) {
            self.push_i32_const(1);
            let result_local = self.allocate_temp_local();
            self.push_local_set(result_local);

            self.emit_numeric_expression(&Expression::Member {
                object: Box::new(actual.clone()),
                property: Box::new(Expression::String("length".to_string())),
            })?;
            self.push_i32_const(expected_binding.values.len() as i32);
            self.push_binary_op(BinaryOp::NotEqual)?;
            self.state.emission.output.instructions.push(0x04);
            self.state
                .emission
                .output
                .instructions
                .push(EMPTY_BLOCK_TYPE);
            self.push_control_frame();
            self.push_i32_const(0);
            self.push_local_set(result_local);
            self.state.emission.output.instructions.push(0x0b);
            self.pop_control_frame();

            for (index, expected_value) in expected_binding.values.iter().enumerate() {
                self.emit_numeric_expression(&Expression::Member {
                    object: Box::new(actual.clone()),
                    property: Box::new(Expression::Number(index as f64)),
                })?;
                self.emit_numeric_expression(
                    &expected_value.clone().unwrap_or(Expression::Undefined),
                )?;
                self.push_binary_op(BinaryOp::NotEqual)?;
                self.state.emission.output.instructions.push(0x04);
                self.state
                    .emission
                    .output
                    .instructions
                    .push(EMPTY_BLOCK_TYPE);
                self.push_control_frame();
                self.push_i32_const(0);
                self.push_local_set(result_local);
                self.state.emission.output.instructions.push(0x0b);
                self.pop_control_frame();
            }

            self.push_local_get(result_local);
            return Ok(true);
        }

        let Some(actual_binding) = self.resolve_array_binding_from_expression(actual) else {
            return Ok(false);
        };
        self.push_i32_const(
            if self.array_bindings_equal(&actual_binding, &expected_binding) {
                1
            } else {
                0
            },
        );
        Ok(true)
    }
}
