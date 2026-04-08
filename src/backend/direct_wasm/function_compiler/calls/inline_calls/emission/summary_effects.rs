use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn emit_inline_user_function_effect_statement(
        &mut self,
        statement: &Statement,
        user_function: &UserFunction,
        call_arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        match statement {
            Statement::Assign { name, value } => {
                self.emit_statement(&Statement::Assign {
                    name: name.clone(),
                    value: self.substitute_user_function_argument_bindings(
                        value,
                        user_function,
                        call_arguments,
                    ),
                })?;
            }
            Statement::Expression(Expression::Update { name, op, prefix }) => {
                self.emit_numeric_expression(&Expression::Update {
                    name: name.clone(),
                    op: *op,
                    prefix: *prefix,
                })?;
                self.state.emission.output.instructions.push(0x1a);
            }
            Statement::Print { values } => {
                self.emit_statement(&Statement::Print {
                    values: values
                        .iter()
                        .map(|value| {
                            self.substitute_user_function_argument_bindings(
                                value,
                                user_function,
                                call_arguments,
                            )
                        })
                        .collect(),
                })?;
            }
            Statement::Expression(expression) => {
                let substituted = self.substitute_user_function_argument_bindings(
                    expression,
                    user_function,
                    call_arguments,
                );
                self.emit_numeric_expression(&substituted)?;
                self.state.emission.output.instructions.push(0x1a);
            }
            Statement::Block { body } if body.is_empty() => {}
            _ => return Ok(false),
        }
        Ok(true)
    }

    pub(super) fn emit_inline_user_function_terminal_statement(
        &mut self,
        terminal_statement: &Statement,
        user_function: &UserFunction,
        call_arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        match terminal_statement {
            Statement::Return(return_value) => {
                let substituted = self.substitute_user_function_argument_bindings(
                    return_value,
                    user_function,
                    call_arguments,
                );
                self.emit_numeric_expression(&substituted)?;
            }
            Statement::Throw(throw_value) => {
                let substituted = self.substitute_user_function_argument_bindings(
                    throw_value,
                    user_function,
                    call_arguments,
                );
                self.emit_statement(&Statement::Throw(substituted))?;
                self.push_i32_const(JS_UNDEFINED_TAG);
            }
            Statement::Assign { name, value } => {
                self.emit_statement(&Statement::Assign {
                    name: name.clone(),
                    value: self.substitute_user_function_argument_bindings(
                        value,
                        user_function,
                        call_arguments,
                    ),
                })?;
                self.push_i32_const(JS_UNDEFINED_TAG);
            }
            Statement::Expression(Expression::Update { name, op, prefix }) => {
                self.emit_numeric_expression(&Expression::Update {
                    name: name.clone(),
                    op: *op,
                    prefix: *prefix,
                })?;
                self.state.emission.output.instructions.push(0x1a);
                self.push_i32_const(JS_UNDEFINED_TAG);
            }
            Statement::Print { values } => {
                self.emit_statement(&Statement::Print {
                    values: values
                        .iter()
                        .map(|value| {
                            self.substitute_user_function_argument_bindings(
                                value,
                                user_function,
                                call_arguments,
                            )
                        })
                        .collect(),
                })?;
                self.push_i32_const(JS_UNDEFINED_TAG);
            }
            Statement::Expression(expression) => {
                let substituted = self.substitute_user_function_argument_bindings(
                    expression,
                    user_function,
                    call_arguments,
                );
                self.emit_numeric_expression(&substituted)?;
                self.state.emission.output.instructions.push(0x1a);
                self.push_i32_const(JS_UNDEFINED_TAG);
            }
            Statement::Block { body } if body.is_empty() => {
                self.push_i32_const(JS_UNDEFINED_TAG);
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_inline_summary_with_call_arguments(
        &mut self,
        user_function: &UserFunction,
        summary: &InlineFunctionSummary,
        call_arguments: &[CallArgument],
    ) -> DirectResult<()> {
        self.with_user_function_execution_context(user_function, |compiler| {
            for effect in &summary.effects {
                match effect {
                    InlineFunctionEffect::Assign { name, value } => {
                        compiler.emit_statement(&Statement::Assign {
                            name: name.clone(),
                            value: compiler.substitute_user_function_argument_bindings(
                                value,
                                user_function,
                                call_arguments,
                            ),
                        })?;
                    }
                    InlineFunctionEffect::Update { name, op, prefix } => {
                        compiler.emit_numeric_expression(&Expression::Update {
                            name: name.clone(),
                            op: *op,
                            prefix: *prefix,
                        })?;
                        compiler.state.emission.output.instructions.push(0x1a);
                    }
                    InlineFunctionEffect::Expression(expression) => {
                        let substituted = compiler.substitute_user_function_argument_bindings(
                            expression,
                            user_function,
                            call_arguments,
                        );
                        compiler.emit_numeric_expression(&substituted)?;
                        compiler.state.emission.output.instructions.push(0x1a);
                    }
                }
            }
            if let Some(return_value) = summary.return_value.as_ref() {
                let substituted = compiler.substitute_user_function_argument_bindings(
                    return_value,
                    user_function,
                    call_arguments,
                );
                compiler.emit_numeric_expression(&substituted)?;
            } else {
                compiler.push_i32_const(JS_UNDEFINED_TAG);
            }
            Ok(())
        })
    }
}
