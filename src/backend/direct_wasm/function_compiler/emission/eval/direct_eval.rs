use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_eval_call(
        &mut self,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        let Some(argument) = arguments.first() else {
            self.push_i32_const(JS_UNDEFINED_TAG);
            return Ok(true);
        };

        let emit_argument_discard =
            |compiler: &mut Self, argument: &CallArgument| -> DirectResult<()> {
                match argument {
                    CallArgument::Expression(expression) => {
                        compiler.emit_numeric_expression(expression)?;
                        compiler.state.emission.output.instructions.push(0x1a);
                    }
                    CallArgument::Spread(expression) => {
                        compiler.emit_numeric_expression(expression)?;
                        compiler.state.emission.output.instructions.push(0x1a);
                    }
                }
                Ok(())
            };

        match argument {
            CallArgument::Expression(expression)
                if self.emit_eval_comment_pattern(expression)? =>
            {
                for argument in arguments.iter().skip(1) {
                    emit_argument_discard(self, argument)?;
                }
                Ok(true)
            }
            CallArgument::Expression(Expression::String(argument_source)) => {
                let raw_source = argument_source.clone();
                let argument_source = if self.state.speculation.execution_context.strict_mode {
                    let mut strict_argument_source = String::from("\"use strict\";");
                    strict_argument_source.push_str(argument_source);
                    Cow::Owned(strict_argument_source)
                } else {
                    Cow::Borrowed(argument_source.as_str())
                };

                for argument in arguments.iter().skip(1) {
                    emit_argument_discard(self, argument)?;
                }

                let mut program = if let Some(program) =
                    self.parse_eval_program_in_current_function_context(&argument_source)
                {
                    program
                } else if let Ok(program) = frontend::parse_script_goal(&argument_source) {
                    program
                } else {
                    self.emit_named_error_throw("SyntaxError")?;
                    return Ok(true);
                };
                namespace_eval_program_internal_function_names(
                    &mut program,
                    self.current_function_name(),
                    &raw_source,
                );
                self.normalize_eval_scoped_bindings_to_source_names(&mut program);

                if self.eval_arguments_declaration_conflicts(&program) {
                    self.emit_named_error_throw("SyntaxError")?;
                    return Ok(true);
                }

                if self.eval_program_declares_var_collision_with_global_lexical(&program) {
                    self.emit_named_error_throw("SyntaxError")?;
                    return Ok(true);
                }

                if self.eval_program_declares_var_collision_with_active_lexical(&program) {
                    self.emit_named_error_throw("SyntaxError")?;
                    return Ok(true);
                }

                if self.eval_program_declares_non_definable_global_function(&program) {
                    self.emit_named_error_throw("TypeError")?;
                    return Ok(true);
                }

                let preexisting_locals = self
                    .state
                    .runtime
                    .locals
                    .keys()
                    .cloned()
                    .collect::<HashSet<_>>();
                let eval_local_function_declarations = if program.strict {
                    HashMap::new()
                } else {
                    collect_eval_local_function_declarations(
                        &program.statements,
                        &program
                            .functions
                            .iter()
                            .filter(|function| is_eval_local_function_candidate(function))
                            .map(|function| function.name.clone())
                            .collect::<HashSet<_>>(),
                    )
                };
                self.prepare_eval_lexical_bindings(
                    &mut program.statements,
                    &eval_local_function_declarations,
                )?;
                self.prepare_eval_var_bindings(&mut program.statements, program.strict)?;
                self.register_bindings_skipping_eval_local_function_declarations(
                    &program.statements,
                    &eval_local_function_declarations,
                )?;
                self.instantiate_eval_var_bindings(&program, &preexisting_locals)?;
                self.instantiate_eval_global_functions(&program.functions)?;
                self.instantiate_eval_local_functions(&eval_local_function_declarations)?;

                self.with_strict_mode(program.strict, |compiler| {
                    compiler.with_active_eval_lexical_scope(
                        collect_direct_eval_lexical_binding_names(&program.statements),
                        |compiler| {
                            let completion_local = compiler.allocate_temp_local();
                            compiler.push_i32_const(JS_UNDEFINED_TAG);
                            compiler.push_local_set(completion_local);
                            let eval_statements = program
                                .statements
                                .iter()
                                .filter(|statement| {
                                    !is_eval_local_function_declaration_statement(
                                        statement,
                                        &eval_local_function_declarations,
                                    )
                                })
                                .collect::<Vec<_>>();

                            for statement in eval_statements {
                                compiler.emit_eval_statement_completion_value(
                                    statement,
                                    completion_local,
                                )?;
                            }

                            compiler.push_local_get(completion_local);

                            Ok(())
                        },
                    )
                })?;

                Ok(true)
            }
            _ => {
                match argument {
                    CallArgument::Expression(expression) => {
                        self.emit_numeric_expression(expression)?
                    }
                    CallArgument::Spread(expression) => {
                        self.emit_numeric_expression(expression)?;
                        self.state.emission.output.instructions.push(0x1a);
                        self.push_i32_const(JS_UNDEFINED_TAG);
                    }
                }

                for argument in arguments.iter().skip(1) {
                    emit_argument_discard(self, argument)?;
                }

                Ok(true)
            }
        }
    }
}
