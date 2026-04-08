use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_hex_quad_string_comparison(
        &mut self,
        left: &Expression,
        right: &Expression,
        op: BinaryOp,
    ) -> DirectResult<bool> {
        let (hex_expression, literal_text) = match (left, right) {
            (expression, Expression::String(text)) => (expression, text.as_str()),
            (Expression::String(text), expression) => (expression, text.as_str()),
            _ => return Ok(false),
        };

        let Some(expected) = parse_fixed_hex_quad(literal_text) else {
            return Ok(false);
        };
        let Some(actual_expression) = self.resolve_hex_quad_numeric_expression(hex_expression)
        else {
            return Ok(false);
        };

        self.emit_numeric_expression(&actual_expression)?;
        self.push_i32_const(expected as i32);
        let comparison = match op {
            BinaryOp::Equal | BinaryOp::LooseEqual => BinaryOp::Equal,
            BinaryOp::NotEqual | BinaryOp::LooseNotEqual => BinaryOp::NotEqual,
            _ => return Ok(false),
        };
        self.push_binary_op(comparison)?;
        Ok(true)
    }
}
