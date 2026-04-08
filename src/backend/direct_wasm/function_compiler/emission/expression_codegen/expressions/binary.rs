use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_binary_expression_value(
        &mut self,
        expression: &Expression,
        op: BinaryOp,
        left: &Expression,
        right: &Expression,
    ) -> DirectResult<()> {
        if matches!(
            op,
            BinaryOp::Subtract
                | BinaryOp::Multiply
                | BinaryOp::Divide
                | BinaryOp::Modulo
                | BinaryOp::Exponentiate
        ) && let Some(number) = self.resolve_static_number_value(expression)
        {
            return self.emit_numeric_expression(&Expression::Number(number));
        }
        match op {
            BinaryOp::Add => {
                let allow_static_addition = !(self.has_current_user_function()
                    && (self.addition_operand_requires_runtime_value(left)
                        || self.addition_operand_requires_runtime_value(right)));
                if allow_static_addition
                    && let Some(outcome) = self.resolve_static_addition_outcome_with_context(
                        left,
                        right,
                        self.current_function_name(),
                    )
                {
                    return self.emit_static_eval_outcome(&outcome);
                }
                if let Some(text) = self.resolve_static_string_addition_value_with_context(
                    left,
                    right,
                    self.current_function_name(),
                ) {
                    self.emit_static_string_literal(&text)?;
                    return Ok(());
                }
                if self.emit_effectful_symbol_to_primitive_addition(left, right)? {
                    return Ok(());
                }
                if self.emit_effectful_ordinary_to_primitive_addition(left, right)? {
                    return Ok(());
                }
                self.emit_numeric_expression(left)?;
                self.emit_numeric_expression(right)?;
                self.push_binary_op(op)
            }
            BinaryOp::LogicalAnd => self.emit_logical_and(left, right),
            BinaryOp::LogicalOr => self.emit_logical_or(left, right),
            BinaryOp::NullishCoalescing => self.emit_nullish_coalescing(left, right),
            BinaryOp::Exponentiate => self.emit_exponentiate(left, right),
            BinaryOp::Equal | BinaryOp::NotEqual
                if self.emit_static_string_equality_comparison(left, right, op)? =>
            {
                Ok(())
            }
            BinaryOp::Equal | BinaryOp::NotEqual
                if self.emit_typeof_string_comparison(left, right, op)? =>
            {
                Ok(())
            }
            BinaryOp::Equal | BinaryOp::NotEqual
                if self.emit_runtime_typeof_tag_string_comparison(left, right, op)? =>
            {
                Ok(())
            }
            BinaryOp::Equal | BinaryOp::NotEqual
                if self.emit_hex_quad_string_comparison(left, right, op)? =>
            {
                Ok(())
            }
            BinaryOp::LooseEqual | BinaryOp::LooseNotEqual
                if self.emit_static_string_equality_comparison(left, right, op)? =>
            {
                Ok(())
            }
            BinaryOp::LooseEqual | BinaryOp::LooseNotEqual
                if self.emit_typeof_string_comparison(left, right, op)? =>
            {
                Ok(())
            }
            BinaryOp::LooseEqual | BinaryOp::LooseNotEqual
                if self.emit_runtime_typeof_tag_string_comparison(left, right, op)? =>
            {
                Ok(())
            }
            BinaryOp::LooseEqual | BinaryOp::LooseNotEqual
                if self.emit_hex_quad_string_comparison(left, right, op)? =>
            {
                Ok(())
            }
            BinaryOp::LooseEqual => {
                self.emit_loose_comparison(left, right)?;
                self.state.emission.output.instructions.push(0x46);
                Ok(())
            }
            BinaryOp::LooseNotEqual => {
                self.emit_loose_comparison(left, right)?;
                self.state.emission.output.instructions.push(0x47);
                Ok(())
            }
            BinaryOp::In => {
                self.emit_in_expression(left, right)?;
                Ok(())
            }
            BinaryOp::InstanceOf => {
                self.emit_instanceof_expression(left, right)?;
                Ok(())
            }
            _ => {
                self.emit_numeric_expression(left)?;
                self.emit_numeric_expression(right)?;
                self.push_binary_op(op)
            }
        }
    }

    pub(in crate::backend::direct_wasm) fn emit_conditional_expression_value(
        &mut self,
        condition: &Expression,
        then_expression: &Expression,
        else_expression: &Expression,
    ) -> DirectResult<()> {
        self.emit_numeric_expression(condition)?;
        self.state.emission.output.instructions.push(0x04);
        self.state.emission.output.instructions.push(I32_TYPE);
        self.push_control_frame();
        self.emit_numeric_expression(then_expression)?;
        self.state.emission.output.instructions.push(0x05);
        self.emit_numeric_expression(else_expression)?;
        self.state.emission.output.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_sequence_expression_value(
        &mut self,
        expressions: &[Expression],
    ) -> DirectResult<()> {
        let Some((last, rest)) = expressions.split_last() else {
            self.push_i32_const(JS_UNDEFINED_TAG);
            return Ok(());
        };
        for expression in rest {
            self.emit_numeric_expression(expression)?;
            self.state.emission.output.instructions.push(0x1a);
        }
        self.emit_numeric_expression(last)
    }
}
