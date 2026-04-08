use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_arguments_slot_accessor_call(
        &mut self,
        callee: &Expression,
        argument_locals: &[u32],
        argument_count: usize,
        inline_arguments: Option<&[Expression]>,
    ) -> DirectResult<bool> {
        let Some(function_binding) = self.resolve_function_binding_from_expression(callee) else {
            return Ok(false);
        };

        match function_binding {
            LocalFunctionBinding::User(function_name) => {
                let Some(user_function) = self.user_function(&function_name).cloned() else {
                    return Ok(false);
                };
                let inline_arguments = inline_arguments
                    .filter(|arguments| arguments.len() == argument_count)
                    .or_else(|| (argument_count == 0).then_some(&[][..]));
                if let Some(inline_arguments) = inline_arguments {
                    if self.can_inline_user_function_call(&user_function, inline_arguments)
                        && self.with_suspended_with_scopes(|compiler| {
                            compiler.emit_inline_user_function_summary_with_arguments(
                                &user_function,
                                inline_arguments,
                            )
                        })?
                    {
                        return Ok(true);
                    }
                }
                if self.with_suspended_with_scopes(|compiler| {
                    compiler.emit_inline_user_function_summary_with_argument_locals(
                        &user_function,
                        argument_locals,
                        argument_count,
                    )
                })? {
                    return Ok(true);
                }
                let visible_param_count = user_function.visible_param_count() as usize;

                for argument_index in 0..visible_param_count {
                    if let Some(argument_local) = argument_locals.get(argument_index).copied() {
                        self.push_local_get(argument_local);
                    } else {
                        self.push_i32_const(JS_UNDEFINED_TAG);
                    }
                }
                self.push_i32_const(argument_count as i32);
                for extra_index in &user_function.extra_argument_indices {
                    if let Some(argument_local) =
                        argument_locals.get(*extra_index as usize).copied()
                    {
                        self.push_local_get(argument_local);
                    } else {
                        self.push_i32_const(JS_UNDEFINED_TAG);
                    }
                }
                self.with_suspended_with_scopes(|compiler| {
                    compiler.push_call(user_function.function_index);
                    let return_value_local = compiler.allocate_temp_local();
                    compiler.push_local_set(return_value_local);
                    compiler.emit_check_global_throw_for_user_call()?;
                    compiler.push_local_get(return_value_local);
                    Ok(())
                })?;
                Ok(true)
            }
            LocalFunctionBinding::Builtin(_) => Ok(false),
        }
    }
}
