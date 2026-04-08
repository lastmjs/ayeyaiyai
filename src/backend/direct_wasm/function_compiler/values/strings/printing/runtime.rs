use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_runtime_print_numeric_value(
        &mut self,
        value: &Expression,
    ) -> DirectResult<()> {
        let value_local = self.allocate_temp_local();
        let handled_local = self.allocate_temp_local();
        self.emit_numeric_expression(value)?;
        self.push_local_set(value_local);
        self.push_i32_const(0);
        self.push_local_set(handled_local);

        for (tag, text) in [(JS_NULL_TAG, "null"), (JS_UNDEFINED_TAG, "undefined")] {
            self.push_local_get(value_local);
            self.push_i32_const(tag);
            self.push_binary_op(BinaryOp::Equal)?;
            self.state.emission.output.instructions.push(0x04);
            self.state
                .emission
                .output
                .instructions
                .push(EMPTY_BLOCK_TYPE);
            self.push_control_frame();
            self.emit_print_string(text)?;
            self.push_i32_const(1);
            self.push_local_set(handled_local);
            self.state.emission.output.instructions.push(0x0b);
            self.pop_control_frame();
        }

        self.push_local_get(handled_local);
        self.state.emission.output.instructions.push(0x45);
        self.state.emission.output.instructions.push(0x04);
        self.state
            .emission
            .output
            .instructions
            .push(EMPTY_BLOCK_TYPE);
        self.push_control_frame();
        self.push_local_get(value_local);
        self.push_i32_const(JS_TYPEOF_NUMBER_TAG);
        self.push_binary_op(BinaryOp::GreaterThanOrEqual)?;
        self.push_local_get(value_local);
        self.push_i32_const(JS_TYPEOF_BIGINT_TAG);
        self.push_binary_op(BinaryOp::LessThanOrEqual)?;
        self.state.emission.output.instructions.push(0x71);
        self.state.emission.output.instructions.push(0x04);
        self.state
            .emission
            .output
            .instructions
            .push(EMPTY_BLOCK_TYPE);
        self.push_control_frame();
        self.emit_typeof_print_from_local(value_local)?;
        self.state.emission.output.instructions.push(0x05);
        self.push_local_get(value_local);
        self.push_i32_const(JS_NAN_TAG);
        self.push_binary_op(BinaryOp::Equal)?;
        self.state.emission.output.instructions.push(0x04);
        self.state
            .emission
            .output
            .instructions
            .push(EMPTY_BLOCK_TYPE);
        self.push_control_frame();
        self.emit_print_string("NaN")?;
        self.state.emission.output.instructions.push(0x05);
        self.push_local_get(value_local);
        self.push_call(PRINT_I32_FUNCTION_INDEX);
        self.state.emission.output.instructions.push(0x0b);
        self.pop_control_frame();
        self.state.emission.output.instructions.push(0x0b);
        self.pop_control_frame();
        self.state.emission.output.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(())
    }
}
