use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn note_last_bound_user_function_source_expression(
        &mut self,
        source_expression: &Expression,
    ) {
        if let Some(snapshot) = self
            .state
            .speculation
            .static_semantics
            .last_bound_user_function_call
            .as_mut()
        {
            snapshot.source_expression = Some(source_expression.clone());
        }
    }

    pub(in crate::backend::direct_wasm) fn emit_ignored_call_arguments(
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
