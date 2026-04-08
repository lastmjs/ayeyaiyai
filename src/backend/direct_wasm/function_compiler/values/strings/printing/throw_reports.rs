use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_uncaught_throw_report_from_locals(
        &mut self,
    ) -> DirectResult<()> {
        let matched_local = self.allocate_temp_local();
        self.push_i32_const(0);
        self.push_local_set(matched_local);

        for name in NATIVE_ERROR_NAMES {
            let Some(value) = native_error_runtime_value(name) else {
                continue;
            };
            self.push_local_get(self.state.runtime.throws.throw_value_local);
            self.push_i32_const(value);
            self.push_binary_op(BinaryOp::Equal)?;
            self.state.emission.output.instructions.push(0x04);
            self.state
                .emission
                .output
                .instructions
                .push(EMPTY_BLOCK_TYPE);
            self.push_control_frame();
            self.emit_stderr_string(name)?;
            self.emit_stderr_string("\n")?;
            self.push_i32_const(1);
            self.push_local_set(matched_local);
            self.state.emission.output.instructions.push(0x0b);
            self.pop_control_frame();
        }

        self.push_local_get(matched_local);
        self.state.emission.output.instructions.push(0x45);
        self.push_local_get(self.state.runtime.throws.throw_tag_local);
        self.push_i32_const(0);
        self.push_binary_op(BinaryOp::NotEqual)?;
        self.state.emission.output.instructions.push(0x71);
        self.state.emission.output.instructions.push(0x04);
        self.state
            .emission
            .output
            .instructions
            .push(EMPTY_BLOCK_TYPE);
        self.push_control_frame();
        self.emit_stderr_string("Error\n")?;
        self.state.emission.output.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(())
    }
}
