use super::*;

fn format_static_number(value: f64) -> String {
    if value.is_nan() {
        "NaN".to_string()
    } else if value.is_infinite() {
        if value.is_sign_positive() {
            "Infinity"
        } else {
            "-Infinity"
        }
        .to_string()
    } else if value == 0.0 && value.is_sign_negative() {
        "-0".to_string()
    } else if value.fract() == 0.0 {
        (value as i64).to_string()
    } else {
        value.to_string()
    }
}

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_print(
        &mut self,
        values: &[Expression],
    ) -> DirectResult<()> {
        let (space_ptr, space_len) = self.intern_string(b" ".to_vec());
        let (newline_ptr, newline_len) = self.intern_string(b"\n".to_vec());

        for (index, value) in values.iter().enumerate() {
            if index > 0 {
                self.push_i32_const(space_ptr as i32);
                self.push_i32_const(space_len as i32);
                self.push_call(WRITE_BYTES_FUNCTION_INDEX);
            }
            self.emit_print_value(value)?;
        }

        self.push_i32_const(newline_ptr as i32);
        self.push_i32_const(newline_len as i32);
        self.push_call(WRITE_BYTES_FUNCTION_INDEX);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_print_value(
        &mut self,
        value: &Expression,
    ) -> DirectResult<()> {
        match value {
            Expression::Number(number) => self.emit_print_string(&format_static_number(*number)),
            Expression::String(text) => {
                let (ptr, len) = self.intern_string(text.as_bytes().to_vec());
                self.push_i32_const(ptr as i32);
                self.push_i32_const(len as i32);
                self.push_call(WRITE_BYTES_FUNCTION_INDEX);
                Ok(())
            }
            Expression::Bool(true) => {
                let (ptr, len) = self.intern_string(b"true".to_vec());
                self.push_i32_const(ptr as i32);
                self.push_i32_const(len as i32);
                self.push_call(WRITE_BYTES_FUNCTION_INDEX);
                Ok(())
            }
            Expression::Bool(false) => {
                let (ptr, len) = self.intern_string(b"false".to_vec());
                self.push_i32_const(ptr as i32);
                self.push_i32_const(len as i32);
                self.push_call(WRITE_BYTES_FUNCTION_INDEX);
                Ok(())
            }
            Expression::Null => {
                let (ptr, len) = self.intern_string(b"null".to_vec());
                self.push_i32_const(ptr as i32);
                self.push_i32_const(len as i32);
                self.push_call(WRITE_BYTES_FUNCTION_INDEX);
                Ok(())
            }
            Expression::Undefined => {
                let (ptr, len) = self.intern_string(b"undefined".to_vec());
                self.push_i32_const(ptr as i32);
                self.push_i32_const(len as i32);
                self.push_call(WRITE_BYTES_FUNCTION_INDEX);
                Ok(())
            }
            Expression::Unary {
                op: UnaryOp::TypeOf,
                expression,
            } => self.emit_typeof_print(expression),
            Expression::Unary {
                op: UnaryOp::Void, ..
            } => {
                let (ptr, len) = self.intern_string(b"undefined".to_vec());
                self.push_i32_const(ptr as i32);
                self.push_i32_const(len as i32);
                self.push_call(WRITE_BYTES_FUNCTION_INDEX);
                Ok(())
            }
            Expression::Unary {
                op: UnaryOp::Delete,
                expression,
            } => {
                match expression.as_ref() {
                    Expression::Identifier(name) => {
                        if self.is_identifier_bound(name) {
                            self.emit_print_string("false")?;
                        } else {
                            self.emit_print_string("true")?;
                        }
                    }
                    Expression::Member { .. }
                    | Expression::SuperMember { .. }
                    | Expression::AssignMember { .. }
                    | Expression::AssignSuperMember { .. }
                    | Expression::This => {
                        self.emit_numeric_expression(expression.as_ref())?;
                        self.state.emission.output.instructions.push(0x1a);
                        self.emit_print_string("true")?;
                    }
                    _ => {
                        self.emit_numeric_expression(expression.as_ref())?;
                        self.state.emission.output.instructions.push(0x1a);
                        self.emit_print_string("true")?;
                    }
                }
                Ok(())
            }
            _ => {
                if let Some(primitive) = self.resolve_static_primitive_expression_with_context(
                    value,
                    self.current_function_name(),
                ) && !static_expression_matches(&primitive, value)
                {
                    if !inline_summary_side_effect_free_expression(value) {
                        self.emit_numeric_expression(value)?;
                        self.state.emission.output.instructions.push(0x1a);
                    }
                    return self.emit_print_value(&primitive);
                }
                if !matches!(
                    value,
                    Expression::Member { .. } | Expression::SuperMember { .. }
                ) && let Some(number) = self.resolve_static_number_value(value)
                    && (number.is_nan()
                        || !number.is_finite()
                        || number.fract() != 0.0
                        || (number == 0.0 && number.is_sign_negative()))
                {
                    return self.emit_print_value(&Expression::Number(number));
                }
                if let Some(text) = self.resolve_static_string_value(value) {
                    self.emit_print_string(&text)?;
                    return Ok(());
                }
                if self.infer_value_kind(value) == Some(StaticValueKind::Bool) {
                    let bool_local = self.allocate_temp_local();
                    self.emit_numeric_expression(value)?;
                    self.push_local_set(bool_local);
                    self.push_local_get(bool_local);
                    self.state.emission.output.instructions.push(0x45);
                    self.state.emission.output.instructions.push(0x04);
                    self.state
                        .emission
                        .output
                        .instructions
                        .push(EMPTY_BLOCK_TYPE);
                    self.push_control_frame();
                    self.emit_print_string("false")?;
                    self.state.emission.output.instructions.push(0x05);
                    self.emit_print_string("true")?;
                    self.state.emission.output.instructions.push(0x0b);
                    self.pop_control_frame();
                    return Ok(());
                }
                self.emit_runtime_print_numeric_value(value)
            }
        }
    }
}
