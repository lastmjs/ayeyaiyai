use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_print_string(
        &mut self,
        text: &str,
    ) -> DirectResult<()> {
        let (ptr, len) = self.intern_string(text.as_bytes().to_vec());
        self.push_i32_const(ptr as i32);
        self.push_i32_const(len as i32);
        self.push_call(WRITE_BYTES_FUNCTION_INDEX);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_write_static_string_to_fd(
        &mut self,
        fd: i32,
        text: &str,
    ) -> DirectResult<()> {
        let (ptr, len) = self.intern_string(text.as_bytes().to_vec());

        self.push_i32_const(IOVEC_OFFSET as i32);
        self.push_i32_const(ptr as i32);
        self.state.emission.output.instructions.push(0x36);
        self.state.emission.output.instructions.push(0x02);
        push_u32(&mut self.state.emission.output.instructions, 0);

        self.push_i32_const((IOVEC_OFFSET + 4) as i32);
        self.push_i32_const(len as i32);
        self.state.emission.output.instructions.push(0x36);
        self.state.emission.output.instructions.push(0x02);
        push_u32(&mut self.state.emission.output.instructions, 0);

        self.push_i32_const(fd);
        self.push_i32_const(IOVEC_OFFSET as i32);
        self.push_i32_const(1);
        self.push_i32_const(NWRITTEN_OFFSET as i32);
        self.push_call(FD_WRITE_FUNCTION_INDEX);
        self.state.emission.output.instructions.push(0x1a);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_stderr_string(
        &mut self,
        text: &str,
    ) -> DirectResult<()> {
        self.emit_write_static_string_to_fd(2, text)
    }

    pub(in crate::backend::direct_wasm) fn emit_static_string_literal(
        &mut self,
        text: &str,
    ) -> DirectResult<()> {
        let (ptr, _) = self.intern_string(text.as_bytes().to_vec());
        self.push_i32_const(ptr as i32);
        Ok(())
    }
}
