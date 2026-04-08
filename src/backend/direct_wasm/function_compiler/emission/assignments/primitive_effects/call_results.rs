use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_binding_call_result_to_local_with_explicit_this(
        &mut self,
        binding: &LocalFunctionBinding,
        arguments: &[Expression],
        this_expression: &Expression,
        this_value: i32,
        result_local: u32,
    ) -> DirectResult<bool> {
        let call_arguments = arguments
            .iter()
            .cloned()
            .map(CallArgument::Expression)
            .collect::<Vec<_>>();
        match binding {
            LocalFunctionBinding::User(function_name) => {
                let Some(user_function) = self.user_function(function_name).cloned() else {
                    return Ok(false);
                };
                if self.can_inline_user_function_call_with_explicit_call_frame(
                    &user_function,
                    arguments,
                    this_expression,
                ) && self.emit_inline_user_function_summary_with_explicit_call_frame(
                    &user_function,
                    arguments,
                    this_expression,
                    result_local,
                )? {
                    return Ok(true);
                }
                self.emit_user_function_call_with_new_target_and_this(
                    &user_function,
                    &call_arguments,
                    JS_UNDEFINED_TAG,
                    this_value,
                )?;
                self.push_local_set(result_local);
                Ok(true)
            }
            LocalFunctionBinding::Builtin(function_name) => {
                if !self.emit_builtin_call(function_name, &call_arguments)? {
                    return Ok(false);
                }
                self.push_local_set(result_local);
                Ok(true)
            }
        }
    }
}
