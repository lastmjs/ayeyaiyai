use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_function_prototype_bind_call(
        &mut self,
        callee: &Expression,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        let Expression::Call {
            callee: bind_callee,
            arguments: bind_arguments,
        } = callee
        else {
            return Ok(false);
        };
        let Expression::Member { object, property } = bind_callee.as_ref() else {
            return Ok(false);
        };
        if !matches!(property.as_ref(), Expression::String(name) if name == "bind") {
            return Ok(false);
        }

        let Some(function_binding) = self.resolve_function_binding_from_expression(object) else {
            return Ok(false);
        };
        if let LocalFunctionBinding::Builtin(function_name) = &function_binding {
            if function_name == "Function.prototype.call"
                && let [
                    CallArgument::Expression(target) | CallArgument::Spread(target),
                    ..,
                ] = bind_arguments.as_slice()
                && let Some(LocalFunctionBinding::Builtin(_)) =
                    self.resolve_function_binding_from_expression(target)
            {
                self.emit_numeric_expression(object)?;
                self.state.emission.output.instructions.push(0x1a);
                for argument in bind_arguments {
                    match argument {
                        CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                            self.emit_numeric_expression(expression)?;
                            self.state.emission.output.instructions.push(0x1a);
                        }
                    }
                }
                for argument in arguments {
                    match argument {
                        CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                            self.emit_numeric_expression(expression)?;
                            self.state.emission.output.instructions.push(0x1a);
                        }
                    }
                }
                self.push_i32_const(JS_TYPEOF_FUNCTION_TAG);
                return Ok(true);
            }
            return Ok(false);
        }
        let LocalFunctionBinding::User(function_name) = function_binding else {
            return Ok(false);
        };
        let Some(user_function) = self.user_function(&function_name).cloned() else {
            return Ok(false);
        };

        let capture_slots = self.resolve_function_expression_capture_slots(object);
        let expanded_bind_arguments = self.expand_call_arguments(bind_arguments);
        let raw_this_expression = expanded_bind_arguments
            .first()
            .cloned()
            .unwrap_or(Expression::Undefined);
        let expanded_call_arguments = self.expand_call_arguments(arguments);
        let bound_call_arguments = expanded_bind_arguments
            .iter()
            .skip(1)
            .cloned()
            .chain(expanded_call_arguments)
            .map(CallArgument::Expression)
            .collect::<Vec<_>>();
        let materialized_this_expression = self.materialize_static_expression(&raw_this_expression);
        let materialized_call_arguments = bound_call_arguments
            .iter()
            .map(|argument| match argument {
                CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                    self.materialize_static_expression(expression)
                }
            })
            .collect::<Vec<_>>();
        let bound_call_argument_expressions = bound_call_arguments
            .iter()
            .map(|argument| match argument {
                CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                    expression.clone()
                }
            })
            .collect::<Vec<_>>();

        self.emit_numeric_expression(object)?;
        self.state.emission.output.instructions.push(0x1a);

        if capture_slots.is_none()
            && (user_function.strict || user_function.lexical_this)
            && self.can_inline_user_function_call_with_explicit_call_frame(
                &user_function,
                &materialized_call_arguments,
                &materialized_this_expression,
            )
        {
            let result_local = self.allocate_temp_local();
            if self.emit_inline_user_function_summary_with_explicit_call_frame(
                &user_function,
                &bound_call_argument_expressions,
                &materialized_this_expression,
                result_local,
            )? {
                self.push_local_get(result_local);
                return Ok(true);
            }
        }

        self.emit_user_function_call_with_function_this_binding(
            &user_function,
            &bound_call_arguments,
            &raw_this_expression,
            capture_slots.as_ref(),
        )?;
        Ok(true)
    }
}
