use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_inline_user_function_effect_statement_with_explicit_call_frame(
        &mut self,
        statement: &Statement,
        user_function: &UserFunction,
        call_arguments: &[CallArgument],
        this_binding: &Expression,
        arguments_binding: &Expression,
        inline_local_bindings: &[String],
    ) -> DirectResult<bool> {
        let mut preserved_descriptor_binding_name = None;
        match statement {
            Statement::Var { name, value } => {
                let substituted_value = self.substitute_user_function_call_frame_bindings(
                    value,
                    user_function,
                    call_arguments,
                    this_binding,
                    arguments_binding,
                );
                if self
                    .resolve_descriptor_binding_from_expression(&substituted_value)
                    .is_some()
                {
                    preserved_descriptor_binding_name = Some(name.clone());
                }
                self.emit_statement(&Statement::Var {
                    name: name.clone(),
                    value: substituted_value,
                })?;
            }
            Statement::Let {
                name,
                mutable,
                value,
            } => {
                let substituted_value = self.substitute_user_function_call_frame_bindings(
                    value,
                    user_function,
                    call_arguments,
                    this_binding,
                    arguments_binding,
                );
                if self
                    .resolve_descriptor_binding_from_expression(&substituted_value)
                    .is_some()
                {
                    preserved_descriptor_binding_name = Some(name.clone());
                }
                self.emit_statement(&Statement::Let {
                    name: name.clone(),
                    mutable: *mutable,
                    value: substituted_value,
                })?;
            }
            Statement::Assign { name, value } => {
                let substituted_value = self.substitute_user_function_call_frame_bindings(
                    value,
                    user_function,
                    call_arguments,
                    this_binding,
                    arguments_binding,
                );
                if self
                    .resolve_descriptor_binding_from_expression(&substituted_value)
                    .is_some()
                {
                    preserved_descriptor_binding_name = Some(name.clone());
                }
                self.emit_statement(&Statement::Assign {
                    name: name.clone(),
                    value: substituted_value,
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
                            self.substitute_user_function_call_frame_bindings(
                                value,
                                user_function,
                                call_arguments,
                                this_binding,
                                arguments_binding,
                            )
                        })
                        .collect(),
                })?;
            }
            Statement::Expression(expression) => {
                let substituted = self.substitute_user_function_call_frame_bindings(
                    expression,
                    user_function,
                    call_arguments,
                    this_binding,
                    arguments_binding,
                );
                if let Expression::Call { callee, arguments } = &substituted
                    && let Expression::Identifier(name) = callee.as_ref()
                {
                    if name == "compareArray" && self.emit_compare_array_call(arguments)? {
                        self.state.emission.output.instructions.push(0x1a);
                        return Ok(true);
                    }
                    if name == "verifyProperty" && self.emit_verify_property_call(arguments)? {
                        self.state.emission.output.instructions.push(0x1a);
                        return Ok(true);
                    }
                }
                self.emit_numeric_expression(&substituted)?;
                self.state.emission.output.instructions.push(0x1a);
            }
            Statement::Throw(throw_value) => {
                let substituted = self.substitute_user_function_call_frame_bindings(
                    throw_value,
                    user_function,
                    call_arguments,
                    this_binding,
                    arguments_binding,
                );
                self.emit_statement(&Statement::Throw(substituted))?;
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.emit_statement(&Statement::If {
                    condition: self.substitute_user_function_call_frame_bindings(
                        condition,
                        user_function,
                        call_arguments,
                        this_binding,
                        arguments_binding,
                    ),
                    then_branch: then_branch
                        .iter()
                        .map(|statement| {
                            self.substitute_statement_call_frame_bindings(
                                statement,
                                user_function,
                                call_arguments,
                                this_binding,
                                arguments_binding,
                            )
                        })
                        .collect::<Vec<_>>(),
                    else_branch: else_branch
                        .iter()
                        .map(|statement| {
                            self.substitute_statement_call_frame_bindings(
                                statement,
                                user_function,
                                call_arguments,
                                this_binding,
                                arguments_binding,
                            )
                        })
                        .collect::<Vec<_>>(),
                })?;
            }
            Statement::Block { body } => {
                for statement in body {
                    if !self.emit_inline_user_function_effect_statement_with_explicit_call_frame(
                        statement,
                        user_function,
                        call_arguments,
                        this_binding,
                        arguments_binding,
                        inline_local_bindings,
                    )? {
                        return Ok(false);
                    }
                }
            }
            _ => return Ok(false),
        }
        if Self::statement_contains_runtime_call(statement) {
            self.invalidate_active_inline_local_descriptor_bindings_except(
                inline_local_bindings,
                preserved_descriptor_binding_name.as_deref(),
            );
        }
        Ok(true)
    }
}
