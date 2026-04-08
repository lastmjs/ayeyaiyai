use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_new_expression(
        &mut self,
        callee: &Expression,
        arguments: &[CallArgument],
    ) -> DirectResult<()> {
        if let Expression::Identifier(name) = callee
            && name == "Proxy"
            && self.is_unshadowed_builtin_identifier(name)
        {
            for argument in arguments {
                match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                        self.emit_numeric_expression(expression)?;
                        self.state.emission.output.instructions.push(0x1a);
                    }
                }
            }
            self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
            return Ok(());
        }

        if let Some(function_binding) = self.resolve_function_binding_from_expression(callee) {
            match function_binding {
                LocalFunctionBinding::User(function_name) => {
                    if let Some(user_function) = self.user_function(&function_name).cloned()
                        && self.emit_user_function_construct(callee, &user_function, arguments)?
                    {
                        return Ok(());
                    }
                }
                LocalFunctionBinding::Builtin(function_name) => {
                    if self.emit_builtin_call_for_callee(callee, &function_name, arguments, true)? {
                        return Ok(());
                    }
                }
            }
        }

        if let Expression::Identifier(name) = callee {
            if self.emit_builtin_call(name, arguments)? {
                return Ok(());
            }

            if let Some(native_error_value) = native_error_runtime_value(name) {
                for argument in arguments {
                    match argument {
                        CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                            self.emit_numeric_expression(expression)?;
                            self.state.emission.output.instructions.push(0x1a);
                        }
                    }
                }
                self.push_i32_const(native_error_value);
                return Ok(());
            }
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
        self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
        Ok(())
    }
}
