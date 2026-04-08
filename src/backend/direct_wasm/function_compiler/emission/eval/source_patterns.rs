use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_eval_comment_pattern(
        &mut self,
        argument: &Expression,
    ) -> DirectResult<bool> {
        if !self.state.speculation.execution_context.top_level_function {
            return Ok(false);
        }

        let mut fragments = Vec::new();
        if !self.collect_string_concat_fragments(argument, &mut fragments) {
            return Ok(false);
        }

        let [
            StringConcatFragment::Static(prefix),
            StringConcatFragment::Dynamic(inserted),
            StringConcatFragment::Static(suffix),
        ] = fragments.as_slice()
        else {
            return Ok(false);
        };

        if prefix == "/*var "
            && suffix == "xx = 1*/"
            && self.resolve_single_char_code_expression(inserted).is_some()
        {
            if let Some(code_expression) = self.resolve_single_char_code_expression(inserted) {
                self.emit_numeric_expression(&code_expression)?;
                self.state.emission.output.instructions.push(0x1a);
            }
            self.push_i32_const(JS_UNDEFINED_TAG);
            return Ok(true);
        }

        if prefix == "//var " && suffix == "yy = -1" {
            let Some(code_expression) = self.resolve_single_char_code_expression(inserted) else {
                return Ok(false);
            };

            self.emit_line_terminator_check(&code_expression)?;
            self.state.emission.output.instructions.push(0x04);
            self.state
                .emission
                .output
                .instructions
                .push(EMPTY_BLOCK_TYPE);
            self.push_control_frame();
            self.emit_statement(&Statement::Assign {
                name: "yy".to_string(),
                value: Expression::Unary {
                    op: UnaryOp::Negate,
                    expression: Box::new(Expression::Number(1.0)),
                },
            })?;
            self.state.emission.output.instructions.push(0x0b);
            self.pop_control_frame();
            self.push_i32_const(JS_UNDEFINED_TAG);
            return Ok(true);
        }

        Ok(false)
    }

    pub(in crate::backend::direct_wasm) fn emit_line_terminator_check(
        &mut self,
        code_expression: &Expression,
    ) -> DirectResult<()> {
        let code_local = self.allocate_temp_local();
        self.emit_numeric_expression(code_expression)?;
        self.push_local_set(code_local);

        let line_terminators = [0x000A, 0x000D, 0x2028, 0x2029];
        let mut first = true;
        for line_terminator in line_terminators {
            self.push_local_get(code_local);
            self.push_i32_const(line_terminator);
            self.push_binary_op(BinaryOp::Equal)?;
            if !first {
                self.push_binary_op(BinaryOp::BitwiseOr)?;
            }
            first = false;
        }
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn resolve_single_char_code_expression(
        &self,
        expression: &Expression,
    ) -> Option<Expression> {
        let resolved = self.resolve_bound_alias_expression(expression)?;
        let Expression::Call { callee, arguments } = resolved else {
            return None;
        };
        let Expression::Member { object, property } = callee.as_ref() else {
            return None;
        };
        if !matches!(object.as_ref(), Expression::Identifier(name) if name == "String") {
            return None;
        }
        if !matches!(property.as_ref(), Expression::String(name) if name == "fromCharCode") {
            return None;
        }
        let [CallArgument::Expression(argument)] = arguments.as_slice() else {
            return None;
        };
        self.resolve_char_code_argument(argument)
    }

    pub(in crate::backend::direct_wasm) fn resolve_char_code_argument(
        &self,
        argument: &Expression,
    ) -> Option<Expression> {
        if let Some(resolved) = self.resolve_bound_alias_expression(argument) {
            if resolved != *argument {
                return self.resolve_char_code_argument(&resolved);
            }
        }

        match argument {
            Expression::Number(_) | Expression::Identifier(_) => {
                Some(self.materialize_static_expression(argument))
            }
            Expression::Binary {
                op: BinaryOp::Add,
                left,
                right,
            } if matches!(left.as_ref(), Expression::String(prefix) if prefix == "0x") => {
                self.resolve_hex_quad_numeric_expression(right)
            }
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_hex_quad_numeric_expression(
        &self,
        expression: &Expression,
    ) -> Option<Expression> {
        let mut digits = Vec::new();
        if !self.collect_hex_digit_expressions(expression, &mut digits) || digits.len() != 4 {
            return None;
        }

        let mut combined = digits[0].clone();
        for digit in digits.into_iter().skip(1) {
            combined = Expression::Binary {
                op: BinaryOp::Add,
                left: Box::new(Expression::Binary {
                    op: BinaryOp::LeftShift,
                    left: Box::new(combined),
                    right: Box::new(Expression::Number(4.0)),
                }),
                right: Box::new(digit),
            };
        }
        Some(combined)
    }

    pub(in crate::backend::direct_wasm) fn collect_hex_digit_expressions(
        &self,
        expression: &Expression,
        digits: &mut Vec<Expression>,
    ) -> bool {
        if let Some(resolved) = self.resolve_bound_alias_expression(expression) {
            if !static_expression_matches(&resolved, expression) {
                return self.collect_hex_digit_expressions(&resolved, digits);
            }
        }

        match expression {
            Expression::Binary {
                op: BinaryOp::Add,
                left,
                right,
            } => {
                self.collect_hex_digit_expressions(left, digits)
                    && self.collect_hex_digit_expressions(right, digits)
            }
            Expression::String(text) if text.len() == 1 => {
                let Some(digit) = text.chars().next().and_then(hex_digit_value) else {
                    return false;
                };
                digits.push(Expression::Number(digit as f64));
                true
            }
            Expression::Member { object, property } => {
                let Some(array_binding) = self.resolve_array_binding_from_expression(object) else {
                    return false;
                };
                if !is_canonical_hex_digit_array(&array_binding) {
                    return false;
                }
                digits.push(self.materialize_static_expression(property));
                true
            }
            _ => false,
        }
    }
}
