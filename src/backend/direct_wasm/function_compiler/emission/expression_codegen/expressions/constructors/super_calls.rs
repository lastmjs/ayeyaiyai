use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_super_call_expression(
        &mut self,
        callee: &Expression,
        arguments: &[CallArgument],
    ) -> DirectResult<()> {
        if let Some(function_binding) = self.resolve_function_binding_from_expression(callee) {
            match function_binding {
                LocalFunctionBinding::User(function_name) => {
                    if let Some(user_function) = self.user_function(&function_name).cloned() {
                        if self.current_function_is_derived_constructor() {
                            self.emit_derived_constructor_super_call(&user_function, arguments)?;
                            return Ok(());
                        }
                        self.emit_user_function_call_with_current_new_target_and_this_expression(
                            &user_function,
                            arguments,
                            &Expression::This,
                        )?;
                        return Ok(());
                    }
                }
                LocalFunctionBinding::Builtin(function_name) => {
                    if self.current_function_is_derived_constructor()
                        && self.emit_derived_constructor_builtin_super_call(
                            &function_name,
                            arguments,
                        )?
                    {
                        return Ok(());
                    }
                    if self.emit_builtin_call(&function_name, arguments)? {
                        return Ok(());
                    }
                }
            }
        }

        if self.emit_dynamic_super_call(callee, arguments)? {
            return Ok(());
        }

        self.emit_numeric_expression(callee)?;
        self.state.emission.output.instructions.push(0x1a);
        for argument in arguments {
            match argument {
                CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                    self.emit_numeric_expression(expression)?;
                }
            }
            self.state.emission.output.instructions.push(0x1a);
        }
        self.push_i32_const(JS_UNDEFINED_TAG);
        Ok(())
    }
}
