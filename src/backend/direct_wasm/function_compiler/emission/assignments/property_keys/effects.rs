use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_property_key_expression_effects(
        &mut self,
        expression: &Expression,
    ) -> DirectResult<Option<Expression>> {
        let resolved = self.resolve_property_key_expression_with_coercion(expression);
        self.emit_numeric_expression(expression)?;
        self.state.emission.output.instructions.push(0x1a);

        if let Some(binding) = resolved
            .as_ref()
            .and_then(|resolved| resolved.coercion.clone())
        {
            match binding {
                LocalFunctionBinding::User(function_name) => {
                    if let Some(user_function) = self.user_function(&function_name).cloned() {
                        self.with_suspended_with_scopes(|compiler| {
                            if compiler.emit_inline_user_function_summary_with_arguments(
                                &user_function,
                                &[],
                            )? {
                                compiler.state.emission.output.instructions.push(0x1a);
                            } else {
                                compiler.emit_user_function_call(&user_function, &[])?;
                                compiler.state.emission.output.instructions.push(0x1a);
                            }
                            Ok(())
                        })?;
                    }
                }
                LocalFunctionBinding::Builtin(function_name) => {
                    self.with_suspended_with_scopes(|compiler| {
                        if compiler.emit_builtin_call(&function_name, &[])? {
                            compiler.state.emission.output.instructions.push(0x1a);
                        }
                        Ok(())
                    })?;
                }
            }
        }

        Ok(resolved.map(|resolved| resolved.key))
    }
}
