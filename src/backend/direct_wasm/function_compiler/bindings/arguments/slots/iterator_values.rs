use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_runtime_array_iterator_value_from_local(
        &mut self,
        index_local: u32,
        values: &[Option<Expression>],
    ) -> DirectResult<()> {
        let mut open_frames = 0;
        for (index, value) in values.iter().enumerate() {
            self.push_local_get(index_local);
            self.push_i32_const(index as i32);
            self.push_binary_op(BinaryOp::Equal)?;
            self.state.emission.output.instructions.push(0x04);
            self.state.emission.output.instructions.push(I32_TYPE);
            self.push_control_frame();
            open_frames += 1;
            if let Some(value) = value {
                self.emit_numeric_expression(value)?;
            } else {
                self.push_i32_const(JS_UNDEFINED_TAG);
            }
            self.state.emission.output.instructions.push(0x05);
        }

        self.push_i32_const(JS_UNDEFINED_TAG);
        for _ in 0..open_frames {
            self.state.emission.output.instructions.push(0x0b);
            self.pop_control_frame();
        }
        Ok(())
    }
}
