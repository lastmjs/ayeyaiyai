use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn register_bindings_skipping_eval_local_function_declarations(
        &mut self,
        statements: &[Statement],
        eval_local_function_declarations: &HashMap<String, String>,
    ) -> DirectResult<()> {
        for statement in statements {
            if is_eval_local_function_declaration_statement(
                statement,
                eval_local_function_declarations,
            ) {
                continue;
            }

            match statement {
                Statement::Declaration { body }
                | Statement::Block { body }
                | Statement::Labeled { body, .. } => self
                    .register_bindings_skipping_eval_local_function_declarations(
                        body,
                        eval_local_function_declarations,
                    )?,
                Statement::Var { name, .. } | Statement::Let { name, .. } => {
                    if self.state.speculation.execution_context.top_level_function
                        && self.backend.global_has_binding(name)
                    {
                        continue;
                    }
                    if self.state.runtime.locals.bindings.contains_key(name) {
                        continue;
                    }
                    let next_local_index = self.state.runtime.locals.next_local_index;
                    self.state
                        .runtime
                        .locals
                        .insert(name.clone(), next_local_index);
                    self.state.runtime.locals.next_local_index += 1;
                }
                Statement::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    self.register_bindings_skipping_eval_local_function_declarations(
                        then_branch,
                        eval_local_function_declarations,
                    )?;
                    self.register_bindings_skipping_eval_local_function_declarations(
                        else_branch,
                        eval_local_function_declarations,
                    )?;
                }
                Statement::With { body, .. } => {
                    self.register_bindings_skipping_eval_local_function_declarations(
                        body,
                        eval_local_function_declarations,
                    )?;
                }
                Statement::While { body, .. } | Statement::DoWhile { body, .. } => self
                    .register_bindings_skipping_eval_local_function_declarations(
                        body,
                        eval_local_function_declarations,
                    )?,
                Statement::Try {
                    body,
                    catch_binding,
                    catch_setup,
                    catch_body,
                    ..
                } => {
                    self.register_bindings_skipping_eval_local_function_declarations(
                        body,
                        eval_local_function_declarations,
                    )?;
                    if let Some(catch_binding) = catch_binding {
                        if !self
                            .state
                            .runtime
                            .locals
                            .bindings
                            .contains_key(catch_binding)
                        {
                            let next_local_index = self.state.runtime.locals.next_local_index;
                            self.state
                                .runtime
                                .locals
                                .insert(catch_binding.clone(), next_local_index);
                            self.state
                                .speculation
                                .static_semantics
                                .set_local_kind(catch_binding, StaticValueKind::Object);
                            self.state.runtime.locals.next_local_index += 1;
                        }
                    }
                    self.register_bindings_skipping_eval_local_function_declarations(
                        catch_setup,
                        eval_local_function_declarations,
                    )?;
                    self.register_bindings_skipping_eval_local_function_declarations(
                        catch_body,
                        eval_local_function_declarations,
                    )?;
                }
                Statement::For {
                    init,
                    per_iteration_bindings,
                    body,
                    ..
                } => {
                    self.register_bindings_skipping_eval_local_function_declarations(
                        init,
                        eval_local_function_declarations,
                    )?;
                    for binding in per_iteration_bindings {
                        if self.state.runtime.locals.bindings.contains_key(binding) {
                            continue;
                        }
                        let next_local_index = self.state.runtime.locals.next_local_index;
                        self.state
                            .runtime
                            .locals
                            .insert(binding.clone(), next_local_index);
                        self.state
                            .speculation
                            .static_semantics
                            .set_local_kind(binding, StaticValueKind::Unknown);
                        self.state.runtime.locals.next_local_index += 1;
                    }
                    self.register_bindings_skipping_eval_local_function_declarations(
                        body,
                        eval_local_function_declarations,
                    )?;
                }
                Statement::Switch {
                    bindings, cases, ..
                } => {
                    for binding in bindings {
                        if self.state.runtime.locals.bindings.contains_key(binding) {
                            continue;
                        }
                        let next_local_index = self.state.runtime.locals.next_local_index;
                        self.state
                            .runtime
                            .locals
                            .insert(binding.clone(), next_local_index);
                        self.state
                            .speculation
                            .static_semantics
                            .set_local_kind(binding, StaticValueKind::Unknown);
                        self.state.runtime.locals.next_local_index += 1;
                    }
                    for case in cases {
                        self.register_bindings_skipping_eval_local_function_declarations(
                            &case.body,
                            eval_local_function_declarations,
                        )?;
                    }
                }
                Statement::Assign { .. }
                | Statement::Break { .. }
                | Statement::Continue { .. }
                | Statement::Expression(_)
                | Statement::Print { .. }
                | Statement::Return(_) => {}
                _ => {}
            }
        }

        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn prepare_eval_lexical_bindings(
        &mut self,
        statements: &mut [Statement],
        eval_local_function_declarations: &HashMap<String, String>,
    ) -> DirectResult<()> {
        let lexical_names = statements
            .iter()
            .filter_map(|statement| match statement {
                Statement::Let { name, .. }
                    if !is_eval_local_function_declaration_statement(
                        statement,
                        eval_local_function_declarations,
                    ) =>
                {
                    Some(name.clone())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        if lexical_names.is_empty() {
            return Ok(());
        }

        let mut renamed_bindings = HashMap::new();
        for name in lexical_names {
            if renamed_bindings.contains_key(&name) {
                continue;
            }
            let hidden_name =
                self.allocate_named_hidden_local("eval_lex", StaticValueKind::Unknown);
            let initialized_local = self.allocate_temp_local();
            let hidden_local = self
                .state
                .runtime
                .locals
                .get(&hidden_name)
                .copied()
                .expect("fresh hidden eval lexical local must exist");
            self.push_i32_const(JS_UNDEFINED_TAG);
            self.push_local_set(hidden_local);
            self.push_i32_const(0);
            self.push_local_set(initialized_local);
            self.state
                .speculation
                .static_semantics
                .eval_lexical_initialized_locals
                .insert(hidden_name.clone(), initialized_local);
            renamed_bindings.insert(name, hidden_name);
        }

        for statement in statements {
            self.rewrite_eval_lexical_statement(statement, &renamed_bindings);
        }

        Ok(())
    }
}
