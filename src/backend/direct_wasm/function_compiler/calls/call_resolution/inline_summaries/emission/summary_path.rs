use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn try_emit_inline_summary_fast_path(
        &mut self,
        user_function: &UserFunction,
        arguments: &[Expression],
        state: &InlineSummaryEmissionState,
        this_binding: &Expression,
        result_local: u32,
    ) -> DirectResult<bool> {
        if user_function.has_lowered_pattern_parameters()
            || self.user_function_contains_local_declaration(user_function)
            || self.user_function_creates_descriptor_binding_with_explicit_call_frame(
                user_function,
                arguments,
                this_binding,
            )
        {
            return Ok(false);
        }
        let Some(summary) = user_function.inline_summary.as_ref() else {
            return Ok(false);
        };
        self.with_user_function_execution_context(user_function, |compiler| {
            for effect in &summary.effects {
                compiler.emit_inline_summary_effect(
                    effect,
                    user_function,
                    &state.call_arguments,
                    this_binding,
                    &state.arguments_binding,
                )?;
            }
            compiler.emit_inline_summary_return_value(
                summary.return_value.as_ref(),
                user_function,
                &state.call_arguments,
                this_binding,
                &state.arguments_binding,
                result_local,
            )
        })?;
        Ok(true)
    }

    fn emit_inline_summary_effect(
        &mut self,
        effect: &InlineFunctionEffect,
        user_function: &UserFunction,
        call_arguments: &[CallArgument],
        this_binding: &Expression,
        arguments_binding: &Expression,
    ) -> DirectResult<()> {
        match effect {
            InlineFunctionEffect::Assign { name, value } => {
                self.emit_statement(&Statement::Assign {
                    name: name.clone(),
                    value: self.substitute_user_function_call_frame_bindings(
                        value,
                        user_function,
                        call_arguments,
                        this_binding,
                        arguments_binding,
                    ),
                })?;
            }
            InlineFunctionEffect::Update { name, op, prefix } => {
                self.emit_numeric_expression(&Expression::Update {
                    name: name.clone(),
                    op: *op,
                    prefix: *prefix,
                })?;
                self.state.emission.output.instructions.push(0x1a);
            }
            InlineFunctionEffect::Expression(expression) => {
                let substituted = self.substitute_user_function_call_frame_bindings(
                    expression,
                    user_function,
                    call_arguments,
                    this_binding,
                    arguments_binding,
                );
                self.emit_numeric_expression(&substituted)?;
                self.state.emission.output.instructions.push(0x1a);
            }
        }
        Ok(())
    }

    fn emit_inline_summary_return_value(
        &mut self,
        return_value: Option<&Expression>,
        user_function: &UserFunction,
        call_arguments: &[CallArgument],
        this_binding: &Expression,
        arguments_binding: &Expression,
        result_local: u32,
    ) -> DirectResult<()> {
        if let Some(return_value) = return_value {
            let substituted = self.substitute_user_function_call_frame_bindings(
                return_value,
                user_function,
                call_arguments,
                this_binding,
                arguments_binding,
            );
            self.emit_numeric_expression(&substituted)?;
            self.push_local_set(result_local);
        } else {
            self.push_i32_const(JS_UNDEFINED_TAG);
            self.push_local_set(result_local);
        }
        Ok(())
    }
}
