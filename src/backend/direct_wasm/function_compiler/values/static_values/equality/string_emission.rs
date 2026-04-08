use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_same_value_result_from_locals(
        &mut self,
        actual_local: u32,
        expected_local: u32,
        result_local: u32,
    ) -> DirectResult<()> {
        self.push_local_get(actual_local);
        self.push_local_get(expected_local);
        self.push_binary_op(BinaryOp::Equal)?;
        self.state.emission.output.instructions.push(0x04);
        self.state
            .emission
            .output
            .instructions
            .push(EMPTY_BLOCK_TYPE);
        self.push_control_frame();
        self.push_i32_const(1);
        self.push_local_set(result_local);
        self.state.emission.output.instructions.push(0x05);
        self.push_local_get(actual_local);
        self.push_i32_const(JS_NAN_TAG);
        self.push_binary_op(BinaryOp::Equal)?;
        self.push_local_get(expected_local);
        self.push_i32_const(JS_NAN_TAG);
        self.push_binary_op(BinaryOp::Equal)?;
        self.state.emission.output.instructions.push(0x71);
        self.push_local_set(result_local);
        self.state.emission.output.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_static_string_equality_comparison(
        &mut self,
        left: &Expression,
        right: &Expression,
        op: BinaryOp,
    ) -> DirectResult<bool> {
        let Some(left_text) = self.resolve_static_string_value(left) else {
            return Ok(false);
        };
        let Some(right_text) = self.resolve_static_string_value(right) else {
            return Ok(false);
        };
        let equal = left_text == right_text;
        let result = match op {
            BinaryOp::Equal | BinaryOp::LooseEqual => equal,
            BinaryOp::NotEqual | BinaryOp::LooseNotEqual => !equal,
            _ => return Ok(false),
        };
        self.push_i32_const(if result { 1 } else { 0 });
        Ok(true)
    }
}
