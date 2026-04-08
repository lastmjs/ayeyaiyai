use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_delete_result_or_throw_if_strict(
        &mut self,
    ) -> DirectResult<()> {
        if !self.state.speculation.execution_context.strict_mode {
            return Ok(());
        }
        let result_local = self.allocate_temp_local();
        self.push_local_set(result_local);
        self.push_local_get(result_local);
        self.state.emission.output.instructions.push(0x45);
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
        self.push_local_get(result_local);
        Ok(())
    }
}
