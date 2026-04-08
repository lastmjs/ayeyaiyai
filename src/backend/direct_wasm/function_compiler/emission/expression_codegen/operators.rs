use super::*;

mod arithmetic;
mod comparisons;
mod logical;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn push_binary_op(
        &mut self,
        op: BinaryOp,
    ) -> DirectResult<()> {
        let opcode = match op {
            BinaryOp::Add => 0x6a,
            BinaryOp::Subtract => 0x6b,
            BinaryOp::Multiply => 0x6c,
            BinaryOp::Divide => 0x6d,
            BinaryOp::Modulo => 0x6f,
            BinaryOp::Equal => 0x46,
            BinaryOp::NotEqual => 0x47,
            BinaryOp::LessThan => 0x48,
            BinaryOp::GreaterThan => 0x4a,
            BinaryOp::LessThanOrEqual => 0x4c,
            BinaryOp::GreaterThanOrEqual => 0x4e,
            BinaryOp::BitwiseAnd => 0x71,
            BinaryOp::BitwiseOr => 0x72,
            BinaryOp::BitwiseXor => 0x73,
            BinaryOp::LeftShift => 0x74,
            BinaryOp::RightShift => 0x75,
            BinaryOp::UnsignedRightShift => 0x76,
            _ => {
                self.push_i32_const(0);
                return Ok(());
            }
        };
        self.state.emission.output.instructions.push(opcode);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn lookup_local(&self, name: &str) -> DirectResult<u32> {
        Ok(self
            .state
            .runtime
            .locals
            .get(name)
            .copied()
            .unwrap_or(self.state.parameters.param_count))
    }
}
