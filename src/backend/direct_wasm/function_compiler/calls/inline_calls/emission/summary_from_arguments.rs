use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_inline_user_function_summary_with_arguments(
        &mut self,
        user_function: &UserFunction,
        arguments: &[Expression],
    ) -> DirectResult<bool> {
        let call_arguments = arguments
            .iter()
            .cloned()
            .map(CallArgument::Expression)
            .collect::<Vec<_>>();

        if let Some(summary) = user_function.inline_summary.as_ref()
            && !self.user_function_contains_local_declaration(user_function)
            && !self
                .user_function_creates_descriptor_binding_with_arguments(user_function, arguments)
        {
            self.emit_inline_summary_with_call_arguments(user_function, summary, &call_arguments)?;
            return Ok(true);
        }

        let Some(function) = self
            .resolve_registered_function_declaration(&user_function.name)
            .cloned()
        else {
            return Ok(false);
        };
        let Some((terminal_statement, effect_statements)) = function.body.split_last() else {
            return Ok(false);
        };

        self.with_user_function_execution_context(user_function, |compiler| {
            for statement in effect_statements {
                if !compiler.emit_inline_user_function_effect_statement(
                    statement,
                    user_function,
                    &call_arguments,
                )? {
                    return Ok(false);
                }
            }
            compiler.emit_inline_user_function_terminal_statement(
                terminal_statement,
                user_function,
                &call_arguments,
            )
        })
    }
}
