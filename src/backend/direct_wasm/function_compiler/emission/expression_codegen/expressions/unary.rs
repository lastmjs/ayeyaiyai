use super::*;

#[path = "unary/call_dispatch.rs"]
mod call_dispatch;
#[path = "unary/delete_ops.rs"]
mod delete_ops;
#[path = "unary/typeof_ops.rs"]
mod typeof_ops;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_unary_expression(
        &mut self,
        op: UnaryOp,
        expression: &Expression,
    ) -> DirectResult<()> {
        match op {
            UnaryOp::TypeOf => self.emit_typeof_expression(expression),
            UnaryOp::Not => {
                self.emit_numeric_expression(expression)?;
                self.state.emission.output.instructions.push(0x45);
                Ok(())
            }
            UnaryOp::BitwiseNot => {
                self.emit_numeric_expression(expression)?;
                self.push_i32_const(-1);
                self.state.emission.output.instructions.push(0x73);
                Ok(())
            }
            UnaryOp::Negate => {
                match expression {
                    Expression::Number(value) if value.is_finite() && value.fract() == 0.0 => {
                        let integer = -(*value as i64);
                        if is_reserved_js_runtime_value(integer) {
                            return Err(Unsupported(
                                "number literal collides with reserved JS tag",
                            ));
                        }
                    }
                    Expression::BigInt(value) => {
                        let integer = format!("-{}", value.strip_suffix('n').unwrap_or(value));
                        if let Ok(parsed) = integer.parse::<i64>()
                            && is_reserved_js_runtime_value(parsed)
                        {
                            return Err(Unsupported(
                                "bigint literal collides with reserved JS tag",
                            ));
                        }
                    }
                    _ => {}
                }
                self.push_i32_const(0);
                self.emit_numeric_expression(expression)?;
                self.state.emission.output.instructions.push(0x6b);
                Ok(())
            }
            UnaryOp::Plus => self.emit_numeric_expression(expression),
            UnaryOp::Void => {
                let temp_local = self.allocate_temp_local();
                self.emit_numeric_expression(expression)?;
                self.push_local_set(temp_local);
                self.push_i32_const(JS_UNDEFINED_TAG);
                Ok(())
            }
            UnaryOp::Delete => self.emit_delete_expression(expression),
        }
    }
}
