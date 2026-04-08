use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_typeof_string_comparison(
        &mut self,
        left: &Expression,
        right: &Expression,
        op: BinaryOp,
    ) -> DirectResult<bool> {
        let (typeof_expression, type_name) = match (left, right) {
            (
                Expression::Unary {
                    op: UnaryOp::TypeOf,
                    expression,
                },
                Expression::String(text),
            ) => (expression.as_ref(), text.as_str()),
            (
                Expression::String(text),
                Expression::Unary {
                    op: UnaryOp::TypeOf,
                    expression,
                },
            ) => (expression.as_ref(), text.as_str()),
            _ => return Ok(false),
        };

        if let Expression::Member { object, property } = typeof_expression
            && self.is_direct_arguments_object(object)
            && matches!(type_name, "undefined")
            && matches!(
                op,
                BinaryOp::Equal
                    | BinaryOp::LooseEqual
                    | BinaryOp::NotEqual
                    | BinaryOp::LooseNotEqual
            )
            && let Some(index) = argument_index_from_expression(property)
        {
            self.emit_arguments_slot_read(index)?;
            self.push_i32_const(JS_UNDEFINED_TAG);
            let comparison = match op {
                BinaryOp::Equal | BinaryOp::LooseEqual => BinaryOp::Equal,
                BinaryOp::NotEqual | BinaryOp::LooseNotEqual => BinaryOp::NotEqual,
                _ => unreachable!("filtered above"),
            };
            self.push_binary_op(comparison)?;
            return Ok(true);
        }

        let Some(type_tag) = parse_typeof_tag_optional(type_name) else {
            self.emit_numeric_expression(typeof_expression)?;
            self.state.emission.output.instructions.push(0x1a);
            self.push_i32_const(match op {
                BinaryOp::Equal | BinaryOp::LooseEqual => 0,
                BinaryOp::NotEqual | BinaryOp::LooseNotEqual => 1,
                _ => return Ok(false),
            });
            return Ok(true);
        };

        self.emit_numeric_expression(&Expression::Unary {
            op: UnaryOp::TypeOf,
            expression: Box::new(typeof_expression.clone()),
        })?;
        self.push_i32_const(type_tag);
        let comparison = match op {
            BinaryOp::Equal | BinaryOp::LooseEqual => BinaryOp::Equal,
            BinaryOp::NotEqual | BinaryOp::LooseNotEqual => BinaryOp::NotEqual,
            _ => return Ok(false),
        };
        self.push_binary_op(comparison)?;
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_runtime_typeof_tag_string_comparison(
        &mut self,
        left: &Expression,
        right: &Expression,
        op: BinaryOp,
    ) -> DirectResult<bool> {
        let (value_expression, type_name) = match (left, right) {
            (expression, Expression::String(text)) => (expression, text.as_str()),
            (Expression::String(text), expression) => (expression, text.as_str()),
            _ => return Ok(false),
        };
        let Some(type_tag) = parse_typeof_tag_optional(type_name) else {
            return Ok(false);
        };

        let value_local = self.allocate_temp_local();
        self.emit_numeric_expression(value_expression)?;
        self.push_local_set(value_local);

        self.push_local_get(value_local);
        self.push_i32_const(JS_TYPEOF_NUMBER_TAG);
        self.push_binary_op(BinaryOp::GreaterThanOrEqual)?;
        self.push_local_get(value_local);
        self.push_i32_const(JS_TYPEOF_BIGINT_TAG);
        self.push_binary_op(BinaryOp::LessThanOrEqual)?;
        self.state.emission.output.instructions.push(0x71);
        self.state.emission.output.instructions.push(0x04);
        self.state.emission.output.instructions.push(I32_TYPE);
        self.push_control_frame();
        self.push_local_get(value_local);
        self.push_i32_const(type_tag);
        let comparison = match op {
            BinaryOp::Equal | BinaryOp::LooseEqual => BinaryOp::Equal,
            BinaryOp::NotEqual | BinaryOp::LooseNotEqual => BinaryOp::NotEqual,
            _ => return Ok(false),
        };
        self.push_binary_op(comparison)?;
        self.state.emission.output.instructions.push(0x05);
        self.push_local_get(value_local);
        self.emit_numeric_expression(&Expression::String(type_name.to_string()))?;
        self.push_binary_op(comparison)?;
        self.state.emission.output.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(true)
    }
}
