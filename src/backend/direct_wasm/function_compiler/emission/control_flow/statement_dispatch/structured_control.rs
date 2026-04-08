use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn emit_structured_statement(&mut self, statement: &Statement) -> DirectResult<()> {
        match statement {
            Statement::Declaration { body } => self.emit_statements(body),
            Statement::Block { body } => self.emit_statements_in_direct_lexical_scope(body),
            Statement::Labeled { labels, body } => self.with_active_eval_lexical_scope(
                collect_direct_eval_lexical_binding_names(body),
                |compiler| compiler.emit_labeled_block(labels, body),
            ),
            Statement::With { object, body } => {
                self.emit_numeric_expression(object)?;
                self.state.emission.output.instructions.push(0x1a);
                let with_scope = self.canonicalize_with_scope_expression(object);
                self.state.push_with_scope(with_scope);
                let result = self.emit_statements(body);
                self.state.pop_with_scope();
                result
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => {
                if !self.if_condition_depends_on_active_loop_assignment(condition)
                    && let Some(condition_value) = self.resolve_static_if_condition_value(condition)
                {
                    if !inline_summary_side_effect_free_expression(condition) {
                        self.emit_numeric_expression(condition)?;
                        self.state.emission.output.instructions.push(0x1a);
                    }
                    if condition_value {
                        self.emit_statements(then_branch)?;
                        for statement in then_branch {
                            self.sync_static_statement_tracking_effects(statement);
                        }
                    } else {
                        self.emit_statements(else_branch)?;
                        for statement in else_branch {
                            self.sync_static_statement_tracking_effects(statement);
                        }
                    }
                    return Ok(());
                }
                self.emit_numeric_expression(condition)?;
                self.state.emission.output.instructions.push(0x04);
                self.state
                    .emission
                    .output
                    .instructions
                    .push(EMPTY_BLOCK_TYPE);
                self.push_control_frame();
                let mut branch_invalidated_bindings = HashSet::new();
                for statement in then_branch {
                    collect_assigned_binding_names_from_statement(
                        statement,
                        &mut branch_invalidated_bindings,
                    );
                }
                for statement in else_branch {
                    collect_assigned_binding_names_from_statement(
                        statement,
                        &mut branch_invalidated_bindings,
                    );
                }
                if let Some((name, narrowed_expression)) =
                    self.conditional_defined_binding_narrowing(condition, true)
                {
                    self.with_restored_static_binding_metadata(|compiler| {
                        compiler.with_narrowed_local_binding_metadata(
                            &name,
                            &narrowed_expression,
                            |compiler| compiler.emit_statements(then_branch),
                        )
                    })?;
                } else {
                    self.with_restored_static_binding_metadata(|compiler| {
                        compiler.emit_statements(then_branch)
                    })?;
                }
                if !else_branch.is_empty() {
                    self.state.emission.output.instructions.push(0x05);
                    if let Some((name, narrowed_expression)) =
                        self.conditional_defined_binding_narrowing(condition, false)
                    {
                        self.with_restored_static_binding_metadata(|compiler| {
                            compiler.with_narrowed_local_binding_metadata(
                                &name,
                                &narrowed_expression,
                                |compiler| compiler.emit_statements(else_branch),
                            )
                        })?;
                    } else {
                        self.with_restored_static_binding_metadata(|compiler| {
                            compiler.emit_statements(else_branch)
                        })?;
                    }
                }
                self.state.emission.output.instructions.push(0x0b);
                self.pop_control_frame();
                self.invalidate_static_binding_metadata_for_names(&branch_invalidated_bindings);
                Ok(())
            }
            Statement::Try {
                body,
                catch_binding,
                catch_setup,
                catch_body,
            } => {
                let static_catch_value = catch_binding
                    .as_ref()
                    .and_then(|_| self.resolve_terminal_throw_value_from_try_body(body));
                self.state.emission.output.instructions.push(0x02);
                self.state
                    .emission
                    .output
                    .instructions
                    .push(EMPTY_BLOCK_TYPE);
                let catch_target = self.push_control_frame();
                self.state
                    .emission
                    .control_flow
                    .try_stack
                    .push(TryContext { catch_target });

                self.emit_statements(body)?;

                self.clear_local_throw_state();
                self.clear_global_throw_state();

                self.state.emission.output.instructions.push(0x0b);
                self.pop_control_frame();
                self.state.emission.control_flow.try_stack.pop();

                self.push_local_get(self.state.runtime.throws.throw_tag_local);
                self.push_i32_const(0);
                self.push_binary_op(BinaryOp::NotEqual)?;
                self.state.emission.output.instructions.push(0x04);
                self.state
                    .emission
                    .output
                    .instructions
                    .push(EMPTY_BLOCK_TYPE);
                self.push_control_frame();

                if let Some(catch_binding) = catch_binding {
                    let catch_local = self.lookup_local(catch_binding)?;
                    self.push_local_get(self.state.runtime.throws.throw_value_local);
                    self.push_local_set(catch_local);
                    let mut invalidated_bindings = HashSet::new();
                    invalidated_bindings.insert(catch_binding.clone());
                    self.invalidate_static_binding_metadata_for_names(&invalidated_bindings);
                    if let Some(static_catch_value) = static_catch_value.as_ref() {
                        self.update_capture_slot_binding_from_expression(
                            catch_binding,
                            static_catch_value,
                        )?;
                    }
                } else {
                    self.push_i32_const(JS_UNDEFINED_TAG);
                    self.push_local_set(self.state.runtime.throws.throw_value_local);
                }

                self.clear_local_throw_state();
                self.clear_global_throw_state();

                let mut catch_scope_bindings =
                    collect_direct_eval_lexical_binding_names(catch_setup);
                catch_scope_bindings.extend(collect_direct_eval_lexical_binding_names(catch_body));
                if let Some(catch_binding) = catch_binding {
                    catch_scope_bindings.push(catch_binding.clone());
                }
                self.with_active_eval_lexical_scope(catch_scope_bindings, |compiler| {
                    if !catch_setup.is_empty() {
                        compiler.emit_statements(catch_setup)?;
                    }
                    if !catch_body.is_empty() {
                        compiler.emit_statements(catch_body)?;
                    }
                    Ok(())
                })?;

                self.state.emission.output.instructions.push(0x0b);
                self.pop_control_frame();
                Ok(())
            }
            Statement::Switch {
                labels,
                bindings,
                discriminant,
                cases,
            } => {
                let mut invalidated_bindings = HashSet::new();
                collect_assigned_binding_names_from_expression(
                    discriminant,
                    &mut invalidated_bindings,
                );
                for case in cases {
                    if let Some(test) = &case.test {
                        collect_assigned_binding_names_from_expression(
                            test,
                            &mut invalidated_bindings,
                        );
                    }
                    for statement in &case.body {
                        collect_assigned_binding_names_from_statement(
                            statement,
                            &mut invalidated_bindings,
                        );
                    }
                }
                self.invalidate_static_binding_metadata_for_names(&invalidated_bindings);
                self.state.emission.output.instructions.push(0x02);
                self.state
                    .emission
                    .output
                    .instructions
                    .push(EMPTY_BLOCK_TYPE);
                let break_target = self.push_control_frame();
                self.state
                    .emission
                    .control_flow
                    .break_stack
                    .push(BreakContext {
                        break_target,
                        labels: labels.to_vec(),
                        break_hook: None,
                    });

                let discriminant_local = self.allocate_temp_local();
                let active_local = self.allocate_temp_local();

                self.emit_numeric_expression(discriminant)?;
                self.push_local_set(discriminant_local);
                self.push_i32_const(0);
                self.push_local_set(active_local);

                self.with_active_eval_lexical_scope(bindings.to_vec(), |compiler| {
                    for case in cases {
                        compiler.emit_switch_case(case, active_local, discriminant_local)?;
                    }
                    Ok(())
                })?;

                self.state.emission.output.instructions.push(0x0b);
                self.pop_control_frame();
                self.state.emission.control_flow.break_stack.pop();
                self.invalidate_static_binding_metadata_for_names(&invalidated_bindings);
                Ok(())
            }
            _ => unreachable!("emit_structured_statement called with non-structured statement"),
        }
    }
}
