use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn try_emit_inline_summary_fallback_path(
        &mut self,
        user_function: &UserFunction,
        state: &InlineSummaryEmissionState,
        this_binding: &Expression,
        result_local: u32,
    ) -> DirectResult<bool> {
        let Some(function) = self
            .resolve_registered_function_declaration(&user_function.name)
            .cloned()
        else {
            return Ok(false);
        };
        let Some((terminal_statement, effect_statements)) = function.body.split_last() else {
            return Ok(false);
        };
        let inline_local_bindings =
            collect_declared_bindings_from_statements_recursive(&function.body)
                .into_iter()
                .filter(|name| {
                    !user_function.params.iter().any(|param| param == name) && name != "arguments"
                })
                .collect::<Vec<_>>();
        let inline_local_scope_names =
            self.prepare_inline_summary_local_bindings(&inline_local_bindings);
        self.with_scoped_lexical_bindings_cleanup(inline_local_scope_names, |compiler| {
            compiler.with_user_function_execution_context(user_function, |compiler| {
                for statement in effect_statements {
                    if !compiler
                        .emit_inline_user_function_effect_statement_with_explicit_call_frame(
                            statement,
                            user_function,
                            &state.call_arguments,
                            this_binding,
                            &state.arguments_binding,
                            &inline_local_bindings,
                        )?
                    {
                        return Ok(false);
                    }
                }
                compiler.emit_inline_summary_terminal_statement(
                    terminal_statement,
                    user_function,
                    &state.call_arguments,
                    this_binding,
                    &state.arguments_binding,
                    result_local,
                )
            })
        })
    }

    fn prepare_inline_summary_local_bindings(
        &mut self,
        inline_local_bindings: &[String],
    ) -> Vec<String> {
        let mut inline_local_scope_names = Vec::new();
        for name in inline_local_bindings {
            let hidden_name = self.allocate_named_hidden_local(
                &format!("inline_local_{name}"),
                StaticValueKind::Unknown,
            );
            let hidden_local = self
                .state
                .runtime
                .locals
                .get(&hidden_name)
                .copied()
                .expect("inline local binding must exist");
            self.push_i32_const(JS_UNDEFINED_TAG);
            self.push_local_set(hidden_local);
            self.state
                .emission
                .lexical_scopes
                .active_scoped_lexical_bindings
                .entry(name.clone())
                .or_default()
                .push(hidden_name);
            inline_local_scope_names.push(name.clone());
        }
        inline_local_scope_names
    }

    fn emit_inline_summary_terminal_statement(
        &mut self,
        terminal_statement: &Statement,
        user_function: &UserFunction,
        call_arguments: &[CallArgument],
        this_binding: &Expression,
        arguments_binding: &Expression,
        result_local: u32,
    ) -> DirectResult<bool> {
        match terminal_statement {
            Statement::Return(return_value) => {
                let substituted = self.substitute_user_function_call_frame_bindings(
                    return_value,
                    user_function,
                    call_arguments,
                    this_binding,
                    arguments_binding,
                );
                self.emit_numeric_expression(&substituted)?;
                self.push_local_set(result_local);
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
            Statement::Var { name, value } => {
                self.emit_statement(&Statement::Var {
                    name: name.clone(),
                    value: self.substitute_user_function_call_frame_bindings(
                        value,
                        user_function,
                        call_arguments,
                        this_binding,
                        arguments_binding,
                    ),
                })?;
                self.push_i32_const(JS_UNDEFINED_TAG);
                self.push_local_set(result_local);
            }
            Statement::Let {
                name,
                mutable,
                value,
            } => {
                self.emit_statement(&Statement::Let {
                    name: name.clone(),
                    mutable: *mutable,
                    value: self.substitute_user_function_call_frame_bindings(
                        value,
                        user_function,
                        call_arguments,
                        this_binding,
                        arguments_binding,
                    ),
                })?;
                self.push_i32_const(JS_UNDEFINED_TAG);
                self.push_local_set(result_local);
            }
            Statement::Assign { name, value } => {
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
                self.push_i32_const(JS_UNDEFINED_TAG);
                self.push_local_set(result_local);
            }
            Statement::Expression(Expression::Update { name, op, prefix }) => {
                self.emit_numeric_expression(&Expression::Update {
                    name: name.clone(),
                    op: *op,
                    prefix: *prefix,
                })?;
                self.state.emission.output.instructions.push(0x1a);
                self.push_i32_const(JS_UNDEFINED_TAG);
                self.push_local_set(result_local);
            }
            Statement::Print { values } => {
                let substituted_values = values
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
                    .collect::<Vec<_>>();
                let (space_ptr, space_len) = self.intern_string(b" ".to_vec());
                let (newline_ptr, newline_len) = self.intern_string(b"\n".to_vec());
                for (index, value) in substituted_values.iter().enumerate() {
                    if index > 0 {
                        self.push_i32_const(space_ptr as i32);
                        self.push_i32_const(space_len as i32);
                        self.push_call(WRITE_BYTES_FUNCTION_INDEX);
                    }
                    match self.infer_value_kind(value) {
                        Some(StaticValueKind::Number | StaticValueKind::BigInt) => {
                            self.emit_runtime_print_numeric_value(value)?;
                        }
                        _ => self.emit_print_value(value)?,
                    }
                }
                self.push_i32_const(newline_ptr as i32);
                self.push_i32_const(newline_len as i32);
                self.push_call(WRITE_BYTES_FUNCTION_INDEX);
                self.push_i32_const(JS_UNDEFINED_TAG);
                self.push_local_set(result_local);
            }
            Statement::Expression(expression) => {
                let substituted = self.substitute_user_function_call_frame_bindings(
                    expression,
                    user_function,
                    call_arguments,
                    this_binding,
                    arguments_binding,
                );
                self.emit_numeric_expression(&substituted)?;
                self.state.emission.output.instructions.push(0x1a);
                self.push_i32_const(JS_UNDEFINED_TAG);
                self.push_local_set(result_local);
            }
            Statement::Block { body } if body.is_empty() => {
                self.push_i32_const(JS_UNDEFINED_TAG);
                self.push_local_set(result_local);
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}
