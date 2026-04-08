use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_same_value_assertion(
        &mut self,
        name: &str,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        let [
            CallArgument::Expression(actual),
            CallArgument::Expression(expected),
            ..,
        ] = arguments
        else {
            return Ok(false);
        };
        let assertion_failure = match name {
            "__assertSameValue" => BinaryOp::NotEqual,
            "__assertNotSameValue" => BinaryOp::Equal,
            _ => return Ok(false),
        };
        let actual_local = self.allocate_temp_local();
        let expected_local = self.allocate_temp_local();
        let handled_as_typeof = matches!(
            (actual, expected),
            (
                Expression::Unary {
                    op: UnaryOp::TypeOf,
                    ..
                },
                Expression::String(_)
            ) | (
                Expression::String(_),
                Expression::Unary {
                    op: UnaryOp::TypeOf,
                    ..
                }
            )
        ) || matches!(
            (actual, expected),
            (Expression::String(text), _) | (_, Expression::String(text))
                if parse_typeof_tag_optional(text).is_some()
        );
        if handled_as_typeof {
            if self.emit_typeof_string_comparison(actual, expected, assertion_failure)?
                || self.emit_runtime_typeof_tag_string_comparison(
                    actual,
                    expected,
                    assertion_failure,
                )?
            {
                self.push_local_set(actual_local);
            } else {
                self.push_i32_const(0);
                self.push_local_set(actual_local);
            }
        } else if !self.assertion_requires_runtime_same_value_fallback()
            && (matches!(actual, Expression::This)
                || matches!(expected, Expression::This)
                || self.resolve_array_binding_from_expression(actual).is_some()
                || self
                    .resolve_array_binding_from_expression(expected)
                    .is_some()
                || self
                    .resolve_object_binding_from_expression(actual)
                    .is_some()
                || self
                    .resolve_object_binding_from_expression(expected)
                    .is_some()
                || self.resolve_user_function_from_expression(actual).is_some()
                || self
                    .resolve_user_function_from_expression(expected)
                    .is_some()
                || (!matches!(actual, Expression::Identifier(_))
                    && !matches!(expected, Expression::Identifier(_))))
            && let Some(result) = self.resolve_static_same_value_result_with_context(
                actual,
                expected,
                self.current_function_name(),
            )
        {
            self.push_i32_const(result as i32);
            self.push_local_set(actual_local);
            if assertion_failure == BinaryOp::NotEqual {
                self.push_local_get(actual_local);
                self.state.emission.output.instructions.push(0x45);
                self.push_local_set(actual_local);
            }
        } else {
            self.emit_numeric_expression(actual)?;
            self.push_local_set(actual_local);
            self.emit_numeric_expression(expected)?;
            self.push_local_set(expected_local);
            self.emit_same_value_result_from_locals(actual_local, expected_local, actual_local)?;
            if assertion_failure == BinaryOp::NotEqual {
                self.push_local_get(actual_local);
                self.state.emission.output.instructions.push(0x45);
                self.push_local_set(actual_local);
            }
        }
        for argument in arguments.iter().skip(2) {
            match argument {
                CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                    self.emit_numeric_expression(expression)?;
                    self.state.emission.output.instructions.push(0x1a);
                }
            }
        }
        self.push_local_get(actual_local);
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
}
