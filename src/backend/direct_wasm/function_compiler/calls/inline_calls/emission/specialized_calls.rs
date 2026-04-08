use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_specialized_function_value_call(
        &mut self,
        specialized: &SpecializedFunctionValue,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        let LocalFunctionBinding::User(function_name) = &specialized.binding else {
            return Ok(false);
        };
        let Some(user_function) = self
            .backend
            .function_registry
            .catalog
            .user_function(function_name)
            .cloned()
        else {
            return Ok(false);
        };
        if user_function.is_async()
            || user_function.is_generator()
            || user_function.has_parameter_defaults()
        {
            return Ok(false);
        }
        let result_expression = specialized
            .summary
            .return_value
            .as_ref()
            .map(|return_value| {
                self.substitute_user_function_argument_bindings(
                    return_value,
                    &user_function,
                    arguments,
                )
            });
        self.state
            .speculation
            .static_semantics
            .last_bound_user_function_call = Some(BoundUserFunctionCallSnapshot {
            function_name: function_name.clone(),
            source_expression: None,
            result_expression,
            updated_bindings: HashMap::new(),
        });
        self.emit_inline_summary_with_call_arguments(
            &user_function,
            &specialized.summary,
            arguments,
        )?;
        Ok(true)
    }
}
