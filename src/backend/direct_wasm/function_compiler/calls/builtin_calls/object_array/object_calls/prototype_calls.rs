use super::*;

#[path = "prototype_calls/query_calls.rs"]
mod query_calls;
#[path = "prototype_calls/set_prototype.rs"]
mod set_prototype;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn discard_call_arguments(
        &mut self,
        arguments: &[CallArgument],
    ) -> DirectResult<()> {
        for argument in arguments {
            match argument {
                CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                    self.emit_numeric_expression(expression)?;
                    self.state.emission.output.instructions.push(0x1a);
                }
            }
        }
        Ok(())
    }
}
