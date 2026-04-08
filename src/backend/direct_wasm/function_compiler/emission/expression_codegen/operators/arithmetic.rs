use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_exponentiate(
        &mut self,
        base: &Expression,
        exponent: &Expression,
    ) -> DirectResult<()> {
        let base_local = self.allocate_temp_local();
        let result_local = self.allocate_temp_local();
        let exponent_local = self.allocate_temp_local();

        self.emit_numeric_expression(base)?;
        self.push_local_set(base_local);

        if let Expression::Number(power) = exponent {
            let power = f64_to_i32(*power)?;
            if power < 0 {
                self.push_i32_const(0);
            } else {
                self.push_i32_const(power);
            }
        } else {
            self.emit_numeric_expression(exponent)?;
        }
        self.push_local_set(exponent_local);

        self.push_i32_const(1);
        self.push_local_set(result_local);

        self.state.emission.output.instructions.push(0x02);
        self.state
            .emission
            .output
            .instructions
            .push(EMPTY_BLOCK_TYPE);
        let break_target = self.push_control_frame();

        self.state.emission.output.instructions.push(0x03);
        self.state
            .emission
            .output
            .instructions
            .push(EMPTY_BLOCK_TYPE);
        let loop_target = self.push_control_frame();

        self.push_local_get(exponent_local);
        self.push_i32_const(0);
        self.push_binary_op(BinaryOp::LessThanOrEqual)?;
        self.push_br_if(self.relative_depth(break_target));

        self.push_local_get(result_local);
        self.push_local_get(base_local);
        self.state.emission.output.instructions.push(0x6c);
        self.push_local_set(result_local);

        self.push_local_get(exponent_local);
        self.push_i32_const(1);
        self.state.emission.output.instructions.push(0x6b);
        self.push_local_set(exponent_local);

        self.push_br(self.relative_depth(loop_target));
        self.state.emission.output.instructions.push(0x0b);
        self.pop_control_frame();
        self.state.emission.output.instructions.push(0x0b);
        self.pop_control_frame();

        self.push_local_get(result_local);
        Ok(())
    }
}
