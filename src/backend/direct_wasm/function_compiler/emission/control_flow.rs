use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_statements(
        &mut self,
        statements: &[Statement],
    ) -> DirectResult<()> {
        for statement in statements {
            self.emit_statement(statement)?;
        }
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_statements_in_direct_lexical_scope(
        &mut self,
        statements: &[Statement],
    ) -> DirectResult<()> {
        self.with_active_eval_lexical_scope(
            collect_direct_eval_lexical_binding_names(statements),
            |compiler| compiler.emit_statements(statements),
        )
    }

    pub(in crate::backend::direct_wasm) fn with_active_eval_lexical_scope<T>(
        &mut self,
        names: Vec<String>,
        body: impl FnOnce(&mut Self) -> DirectResult<T>,
    ) -> DirectResult<T> {
        self.push_active_eval_lexical_scope(names);
        let result = body(self);
        self.pop_active_eval_lexical_scope();
        result
    }

    pub(in crate::backend::direct_wasm) fn push_active_eval_lexical_scope(
        &mut self,
        names: Vec<String>,
    ) {
        let mut pushed = Vec::new();
        let mut seen = HashSet::new();
        for name in names {
            let source_name = scoped_binding_source_name(&name)
                .unwrap_or(&name)
                .to_string();
            if !seen.insert(source_name.clone()) {
                continue;
            }
            *self
                .active_eval_lexical_binding_counts
                .entry(source_name.clone())
                .or_insert(0) += 1;
            let active_binding =
                (name != source_name && self.locals.contains_key(&name)).then_some(name.clone());
            if let Some(active_binding) = active_binding.as_ref() {
                self.active_scoped_lexical_bindings
                    .entry(source_name.clone())
                    .or_default()
                    .push(active_binding.clone());
            }
            pushed.push((source_name, active_binding));
        }
        self.active_eval_lexical_scopes.push(pushed);
    }

    pub(in crate::backend::direct_wasm) fn pop_active_eval_lexical_scope(&mut self) {
        let Some(names) = self.active_eval_lexical_scopes.pop() else {
            return;
        };
        for (name, active_binding) in names {
            let Some(count) = self.active_eval_lexical_binding_counts.get_mut(&name) else {
                continue;
            };
            *count -= 1;
            if *count == 0 {
                self.active_eval_lexical_binding_counts.remove(&name);
            }
            if let Some(active_binding) = active_binding
                && let Some(bindings) = self.active_scoped_lexical_bindings.get_mut(&name)
            {
                if bindings
                    .last()
                    .is_some_and(|binding| binding == &active_binding)
                {
                    bindings.pop();
                } else if let Some(index) = bindings
                    .iter()
                    .rposition(|binding| binding == &active_binding)
                {
                    bindings.remove(index);
                }
                if bindings.is_empty() {
                    self.active_scoped_lexical_bindings.remove(&name);
                }
            }
        }
    }

    pub(in crate::backend::direct_wasm) fn emit_statement(
        &mut self,
        statement: &Statement,
    ) -> DirectResult<()> {
        match statement {
            Statement::Block { body } => self.emit_statements_in_direct_lexical_scope(body),
            Statement::Labeled { labels, body } => self.with_active_eval_lexical_scope(
                collect_direct_eval_lexical_binding_names(body),
                |compiler| compiler.emit_labeled_block(labels, body),
            ),
            Statement::Var { name, value } => {
                let value_local = self.allocate_temp_local();
                let scoped_target = self.resolve_with_scope_binding(name)?;
                self.emit_numeric_expression(value)?;
                self.push_local_set(value_local);
                if let Some(scope_object) = scoped_target {
                    self.emit_scoped_property_store_from_local(
                        &scope_object,
                        name,
                        value_local,
                        value,
                    )?;
                    self.instructions.push(0x1a);
                } else {
                    self.emit_store_identifier_value_local(name, value, value_local)?;
                }
                Ok(())
            }
            Statement::Let { name, value, .. } => {
                let value_local = self.allocate_temp_local();
                self.emit_numeric_expression(value)?;
                self.push_local_set(value_local);
                self.emit_store_identifier_value_local(name, value, value_local)?;
                if let Some(initialized_local) =
                    self.eval_lexical_initialized_locals.get(name).copied()
                {
                    self.push_i32_const(1);
                    self.push_local_set(initialized_local);
                }
                Ok(())
            }
            Statement::Assign { name, value } => {
                if self.try_emit_destructuring_default_assign_statement(name, value)? {
                    return Ok(());
                }
                let scoped_target = self.resolve_with_scope_binding(name)?;
                self.emit_numeric_expression(value)?;
                if let Some(scope_object) = scoped_target {
                    let value_local = self.allocate_temp_local();
                    self.push_local_set(value_local);
                    self.emit_scoped_property_store_from_local(
                        &scope_object,
                        name,
                        value_local,
                        value,
                    )?;
                    self.instructions.push(0x1a);
                } else {
                    let value_local = self.allocate_temp_local();
                    self.push_local_set(value_local);
                    self.emit_store_identifier_value_local(name, value, value_local)?;
                }
                Ok(())
            }
            Statement::AssignMember {
                object,
                property,
                value,
            } => {
                self.emit_numeric_expression(&Expression::AssignMember {
                    object: Box::new(object.clone()),
                    property: Box::new(property.clone()),
                    value: Box::new(value.clone()),
                })?;
                self.instructions.push(0x1a);
                Ok(())
            }
            Statement::Expression(expression) => {
                if self.emit_assert_throws_statement(expression)? {
                    return Ok(());
                }
                if let Expression::Call { callee, arguments } = expression
                    && arguments.is_empty()
                    && let Expression::Member { object, property } = callee.as_ref()
                    && matches!(property.as_ref(), Expression::String(name) if name == "next")
                    && matches!(object.as_ref(), Expression::Identifier(name) if self.local_array_iterator_bindings.contains_key(name))
                {
                    let hidden_name = self.allocate_named_hidden_local(
                        "direct_iterator_step_stmt",
                        StaticValueKind::Object,
                    );
                    self.update_local_iterator_step_binding(&hidden_name, expression);
                    self.emit_numeric_expression(object)?;
                    self.instructions.push(0x1a);
                    self.update_member_function_binding_from_expression(expression);
                    self.update_object_binding_from_expression(expression);
                    return Ok(());
                }
                if let Expression::Call { callee, arguments } = expression
                    && arguments.is_empty()
                    && let Expression::Member { object, property } = callee.as_ref()
                    && matches!(property.as_ref(), Expression::String(name) if name == "next")
                    && self.emit_fresh_simple_generator_next_call(object)?
                {
                    self.update_member_function_binding_from_expression(expression);
                    self.update_object_binding_from_expression(expression);
                    self.instructions.push(0x1a);
                    return Ok(());
                }
                self.emit_numeric_expression(expression)?;
                self.update_member_function_binding_from_expression(expression);
                self.update_object_binding_from_expression(expression);
                self.instructions.push(0x1a);
                Ok(())
            }
            Statement::Print { values } => self.emit_print(values),
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
                        self.instructions.push(0x1a);
                    }
                    if condition_value {
                        self.emit_statements(then_branch)?;
                    } else {
                        self.emit_statements(else_branch)?;
                    }
                    return Ok(());
                }
                self.emit_numeric_expression(condition)?;
                self.instructions.push(0x04);
                self.instructions.push(EMPTY_BLOCK_TYPE);
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
                    self.instructions.push(0x05);
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
                self.instructions.push(0x0b);
                self.pop_control_frame();
                self.invalidate_static_binding_metadata_for_names(&branch_invalidated_bindings);
                Ok(())
            }
            Statement::While {
                condition,
                body,
                break_hook,
                labels,
            } => self.emit_while(condition, break_hook.as_ref(), labels, body),
            Statement::DoWhile {
                condition,
                body,
                break_hook,
                labels,
            } => self.emit_do_while(condition, break_hook.as_ref(), labels, body),
            Statement::For {
                init,
                condition,
                update,
                break_hook,
                labels,
                body,
                per_iteration_bindings,
            } => self.emit_for(
                labels,
                init,
                per_iteration_bindings,
                condition.as_ref(),
                update.as_ref(),
                break_hook.as_ref(),
                body,
            ),
            Statement::Break { label } => {
                let target_index = if let Some(label) = label.as_ref() {
                    match self.find_labeled_break(label)? {
                        Some(index) => index,
                        None => return Ok(()),
                    }
                } else {
                    match self.break_stack.len().checked_sub(1) {
                        Some(index) => index,
                        None => return Ok(()),
                    }
                };

                for context_index in (target_index..self.break_stack.len()).rev() {
                    let break_hook =
                        self.break_hook_for_target(self.break_stack[context_index].break_target)?;
                    if let Some(break_hook) = break_hook {
                        self.emit_numeric_expression(&break_hook)?;
                        self.instructions.push(0x1a);
                    }
                }

                let break_target = self.break_stack[target_index].break_target;
                self.push_br(self.relative_depth(break_target));
                Ok(())
            }
            Statement::Continue { label } => {
                if label.is_some() {
                    let label = label
                        .as_ref()
                        .expect("labeled continue branch should include label");
                    let target_index = match self.find_labeled_loop_index(label)? {
                        Some(index) => index,
                        None => return Ok(()),
                    };
                    if target_index == self.loop_stack.len() - 1 {
                        let (continue_target, break_target) = {
                            let Some(loop_context) = self.loop_stack.last() else {
                                return Ok(());
                            };
                            (loop_context.continue_target, loop_context.break_target)
                        };
                        let break_hook = self.break_hook_for_target(break_target)?;
                        if let Some(break_hook) = break_hook {
                            self.emit_numeric_expression(&break_hook)?;
                            self.instructions.push(0x1a);
                        }
                        self.push_br(self.relative_depth(continue_target));
                        return Ok(());
                    }

                    for loop_index in (target_index + 1..self.loop_stack.len()).rev() {
                        let break_target = self.loop_stack[loop_index].break_target;
                        if let Some(break_hook) = self.break_hook_for_target(break_target)? {
                            self.emit_numeric_expression(&break_hook)?;
                            self.instructions.push(0x1a);
                        }
                    }

                    let target = self.loop_stack[target_index].continue_target;
                    self.push_br(self.relative_depth(target));
                    return Ok(());
                }
                let Some(loop_context) = self.loop_stack.last() else {
                    return Ok(());
                };
                let (continue_target, break_target) =
                    { (loop_context.continue_target, loop_context.break_target) };
                let break_hook = self.break_hook_for_target(break_target)?;
                if let Some(break_hook) = break_hook {
                    self.emit_numeric_expression(&break_hook)?;
                    self.instructions.push(0x1a);
                }
                self.push_br(self.relative_depth(continue_target));
                Ok(())
            }
            Statement::Return(expression) => {
                if !self.allow_return {
                    self.emit_numeric_expression(expression)?;
                    self.instructions.push(0x1a);
                    self.push_i32_const(JS_UNDEFINED_TAG);
                    return Ok(());
                }
                self.emit_numeric_expression(expression)?;
                self.clear_local_throw_state();
                self.clear_global_throw_state();
                self.instructions.push(0x0f);
                Ok(())
            }
            Statement::Throw(expression) => {
                self.emit_numeric_expression(expression)?;
                self.push_local_set(self.throw_value_local);
                self.push_i32_const(1);
                self.push_local_set(self.throw_tag_local);
                self.emit_throw_from_locals()
            }
            Statement::With { object, body } => {
                self.emit_numeric_expression(object)?;
                self.instructions.push(0x1a);
                let with_scope = self.canonicalize_with_scope_expression(object);
                self.with_scopes.push(with_scope);
                let result = self.emit_statements(body);
                self.with_scopes.pop();
                result
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
                self.instructions.push(0x02);
                self.instructions.push(EMPTY_BLOCK_TYPE);
                let catch_target = self.push_control_frame();
                self.try_stack.push(TryContext { catch_target });

                self.emit_statements(body)?;

                self.clear_local_throw_state();
                self.clear_global_throw_state();

                self.instructions.push(0x0b);
                self.pop_control_frame();
                self.try_stack.pop();

                self.push_local_get(self.throw_tag_local);
                self.push_i32_const(0);
                self.push_binary_op(BinaryOp::NotEqual)?;
                self.instructions.push(0x04);
                self.instructions.push(EMPTY_BLOCK_TYPE);
                self.push_control_frame();

                if let Some(catch_binding) = catch_binding {
                    let catch_local = self.lookup_local(catch_binding)?;
                    self.push_local_get(self.throw_value_local);
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
                    self.push_local_set(self.throw_value_local);
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

                self.instructions.push(0x0b);
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
                self.instructions.push(0x02);
                self.instructions.push(EMPTY_BLOCK_TYPE);
                let break_target = self.push_control_frame();
                self.break_stack.push(BreakContext {
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

                self.instructions.push(0x0b);
                self.pop_control_frame();
                self.break_stack.pop();
                self.invalidate_static_binding_metadata_for_names(&invalidated_bindings);
                Ok(())
            }
            Statement::Yield { value } => {
                self.emit_numeric_expression(value)?;
                self.instructions.push(0x00);
                Ok(())
            }
            Statement::YieldDelegate { value } => {
                self.emit_numeric_expression(value)?;
                self.instructions.push(0x00);
                Ok(())
            }
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_terminal_throw_value_from_try_body(
        &self,
        body: &[Statement],
    ) -> Option<Expression> {
        let [statement] = body else {
            return None;
        };
        match statement {
            Statement::Block { body } => self.resolve_terminal_throw_value_from_try_body(body),
            Statement::Throw(expression) => Some(expression.clone()),
            Statement::Expression(expression) => {
                self.resolve_terminal_expression_throw_value(expression)
            }
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn emit_throw_from_locals(&mut self) -> DirectResult<()> {
        self.push_local_get(self.throw_value_local);
        self.push_global_set(THROW_VALUE_GLOBAL_INDEX);
        self.push_local_get(self.throw_tag_local);
        self.push_global_set(THROW_TAG_GLOBAL_INDEX);

        let Some(try_context) = self.try_stack.last() else {
            if self.allow_return {
                self.push_i32_const(JS_UNDEFINED_TAG);
                self.clear_local_throw_state();
                self.instructions.push(0x0f);
                return Ok(());
            }
            self.emit_uncaught_throw_report_from_locals()?;
            self.instructions.push(0x00);
            return Ok(());
        };

        self.push_br(self.relative_depth(try_context.catch_target));
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_labeled_block(
        &mut self,
        labels: &[String],
        body: &[Statement],
    ) -> DirectResult<()> {
        self.instructions.push(0x02);
        self.instructions.push(EMPTY_BLOCK_TYPE);
        let break_target = self.push_control_frame();
        self.break_stack.push(BreakContext {
            break_target,
            labels: labels.to_vec(),
            break_hook: None,
        });
        self.emit_statements(body)?;
        self.instructions.push(0x0b);
        self.pop_control_frame();
        self.break_stack.pop();
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_check_global_throw_for_user_call(
        &mut self,
    ) -> DirectResult<()> {
        self.push_global_get(THROW_TAG_GLOBAL_INDEX);
        self.push_i32_const(0);
        self.push_binary_op(BinaryOp::NotEqual)?;
        self.instructions.push(0x04);
        self.instructions.push(EMPTY_BLOCK_TYPE);
        self.push_control_frame();

        self.push_global_get(THROW_VALUE_GLOBAL_INDEX);
        self.push_local_set(self.throw_value_local);
        self.push_global_get(THROW_TAG_GLOBAL_INDEX);
        self.push_local_set(self.throw_tag_local);

        let Some(catch_target) = self
            .try_stack
            .last()
            .map(|try_context| try_context.catch_target)
        else {
            if self.allow_return {
                self.push_i32_const(JS_UNDEFINED_TAG);
                self.instructions.push(0x0f);
            } else {
                self.emit_uncaught_throw_report_from_locals()?;
                self.instructions.push(0x00);
            }
            self.instructions.push(0x0b);
            self.pop_control_frame();
            return Ok(());
        };

        self.clear_global_throw_state();
        self.push_br(self.relative_depth(catch_target));
        self.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn clear_local_throw_state(&mut self) {
        self.push_i32_const(0);
        self.push_local_set(self.throw_tag_local);
        self.push_i32_const(0);
        self.push_local_set(self.throw_value_local);
    }

    pub(in crate::backend::direct_wasm) fn clear_global_throw_state(&mut self) {
        self.push_i32_const(0);
        self.push_global_set(THROW_TAG_GLOBAL_INDEX);
        self.push_i32_const(0);
        self.push_global_set(THROW_VALUE_GLOBAL_INDEX);
    }

    pub(in crate::backend::direct_wasm) fn emit_error_throw(&mut self) -> DirectResult<()> {
        self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
        self.push_local_set(self.throw_value_local);
        self.push_i32_const(1);
        self.push_local_set(self.throw_tag_local);
        self.emit_throw_from_locals()
    }

    pub(in crate::backend::direct_wasm) fn emit_named_error_throw(
        &mut self,
        name: &str,
    ) -> DirectResult<()> {
        if let Some(value) = native_error_runtime_value(name) {
            self.push_i32_const(value);
            self.push_local_set(self.throw_value_local);
            self.push_i32_const(1);
            self.push_local_set(self.throw_tag_local);
            return self.emit_throw_from_locals();
        }

        self.emit_error_throw()
    }

    pub(in crate::backend::direct_wasm) fn emit_static_throw_value(
        &mut self,
        throw_value: &StaticThrowValue,
    ) -> DirectResult<()> {
        match throw_value {
            StaticThrowValue::Value(expression) => {
                self.emit_numeric_expression(expression)?;
                self.push_local_set(self.throw_value_local);
                self.push_i32_const(1);
                self.push_local_set(self.throw_tag_local);
                self.emit_throw_from_locals()
            }
            StaticThrowValue::NamedError(name) => self.emit_named_error_throw(name),
        }
    }

    pub(in crate::backend::direct_wasm) fn emit_static_eval_outcome(
        &mut self,
        outcome: &StaticEvalOutcome,
    ) -> DirectResult<()> {
        match outcome {
            StaticEvalOutcome::Value(expression) => self.emit_numeric_expression(expression),
            StaticEvalOutcome::Throw(throw_value) => self.emit_static_throw_value(throw_value),
        }
    }

    pub(in crate::backend::direct_wasm) fn emit_while(
        &mut self,
        condition: &Expression,
        break_hook: Option<&Expression>,
        labels: &[String],
        body: &[Statement],
    ) -> DirectResult<()> {
        let invalidated_bindings =
            collect_loop_assigned_binding_names(condition, break_hook, body, None, None);
        let preserved_kinds = self.preserved_binding_kinds_for_loop(
            &invalidated_bindings,
            condition,
            break_hook,
            body,
            None,
        );
        self.invalidate_static_binding_metadata_for_names_with_preserved_kinds(
            &invalidated_bindings,
            &preserved_kinds,
        );
        self.instructions.push(0x02);
        self.instructions.push(EMPTY_BLOCK_TYPE);
        let break_target = self.push_control_frame();

        self.instructions.push(0x03);
        self.instructions.push(EMPTY_BLOCK_TYPE);
        let continue_target = self.push_control_frame();
        self.loop_stack.push(LoopContext {
            break_target,
            continue_target,
            labels: labels.to_vec(),
            assigned_bindings: invalidated_bindings.clone(),
        });
        self.break_stack.push(BreakContext {
            break_target,
            labels: labels.to_vec(),
            break_hook: break_hook.cloned(),
        });

        self.emit_numeric_expression(condition)?;
        self.instructions.push(0x45);
        self.push_br_if(self.relative_depth(break_target));
        self.emit_statements(body)?;
        self.push_br(self.relative_depth(continue_target));

        self.loop_stack.pop();
        self.break_stack.pop();
        self.instructions.push(0x0b);
        self.pop_control_frame();
        self.instructions.push(0x0b);
        self.pop_control_frame();
        self.invalidate_static_binding_metadata_for_names_with_preserved_kinds(
            &invalidated_bindings,
            &preserved_kinds,
        );
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn conditional_defined_binding_narrowing(
        &self,
        condition: &Expression,
        then_branch: bool,
    ) -> Option<(String, Expression)> {
        let (name, defined_when_condition_true) = match condition {
            Expression::Binary {
                op: BinaryOp::NotEqual,
                left,
                right,
            } if matches!(right.as_ref(), Expression::Undefined) => {
                let Expression::Identifier(name) = left.as_ref() else {
                    return None;
                };
                (name.clone(), true)
            }
            Expression::Binary {
                op: BinaryOp::NotEqual,
                left,
                right,
            } if matches!(left.as_ref(), Expression::Undefined) => {
                let Expression::Identifier(name) = right.as_ref() else {
                    return None;
                };
                (name.clone(), true)
            }
            Expression::Binary {
                op: BinaryOp::Equal,
                left,
                right,
            } if matches!(right.as_ref(), Expression::Undefined) => {
                let Expression::Identifier(name) = left.as_ref() else {
                    return None;
                };
                (name.clone(), false)
            }
            Expression::Binary {
                op: BinaryOp::Equal,
                left,
                right,
            } if matches!(left.as_ref(), Expression::Undefined) => {
                let Expression::Identifier(name) = right.as_ref() else {
                    return None;
                };
                (name.clone(), false)
            }
            _ => return None,
        };

        let Expression::Conditional {
            then_expression,
            else_expression,
            ..
        } = self.local_value_bindings.get(&name)?
        else {
            return None;
        };

        let then_is_undefined = matches!(then_expression.as_ref(), Expression::Undefined);
        let else_is_undefined = matches!(else_expression.as_ref(), Expression::Undefined);
        if then_is_undefined == else_is_undefined {
            return None;
        }

        let defined_expression = if !then_is_undefined {
            then_expression.as_ref().clone()
        } else {
            else_expression.as_ref().clone()
        };
        let branch_expression = if then_branch == defined_when_condition_true {
            defined_expression
        } else {
            Expression::Undefined
        };
        Some((name, branch_expression))
    }

    pub(in crate::backend::direct_wasm) fn expression_depends_on_active_loop_assignment(
        &self,
        expression: &Expression,
    ) -> bool {
        let mut referenced_names = HashSet::new();
        collect_referenced_binding_names_from_expression(expression, &mut referenced_names);
        self.loop_stack.iter().rev().any(|loop_context| {
            referenced_names.iter().any(|name| {
                loop_context.assigned_bindings.contains(name)
                    || scoped_binding_source_name(name).is_some_and(|source_name| {
                        loop_context.assigned_bindings.contains(source_name)
                    })
            })
        })
    }

    pub(in crate::backend::direct_wasm) fn if_condition_depends_on_active_loop_assignment(
        &self,
        condition: &Expression,
    ) -> bool {
        self.expression_depends_on_active_loop_assignment(condition)
    }

    pub(in crate::backend::direct_wasm) fn with_restored_static_binding_metadata<T>(
        &mut self,
        callback: impl FnOnce(&mut Self) -> DirectResult<T>,
    ) -> DirectResult<T> {
        let saved_local_kinds = self.local_kinds.clone();
        let saved_local_value_bindings = self.local_value_bindings.clone();
        let saved_local_function_bindings = self.local_function_bindings.clone();
        let saved_local_specialized_function_values =
            self.local_specialized_function_values.clone();
        let saved_local_proxy_bindings = self.local_proxy_bindings.clone();
        let saved_member_function_bindings = self.member_function_bindings.clone();
        let saved_member_function_capture_slots = self.member_function_capture_slots.clone();
        let saved_member_getter_bindings = self.member_getter_bindings.clone();
        let saved_member_setter_bindings = self.member_setter_bindings.clone();
        let saved_local_array_bindings = self.local_array_bindings.clone();
        let saved_local_resizable_array_buffer_bindings =
            self.local_resizable_array_buffer_bindings.clone();
        let saved_local_typed_array_view_bindings = self.local_typed_array_view_bindings.clone();
        let saved_runtime_typed_array_oob_locals = self.runtime_typed_array_oob_locals.clone();
        let saved_tracked_array_function_values = self.tracked_array_function_values.clone();
        let saved_runtime_array_slots = self.runtime_array_slots.clone();
        let saved_local_array_iterator_bindings = self.local_array_iterator_bindings.clone();
        let saved_local_iterator_step_bindings = self.local_iterator_step_bindings.clone();
        let saved_runtime_array_length_locals = self.runtime_array_length_locals.clone();
        let saved_local_object_bindings = self.local_object_bindings.clone();
        let saved_local_prototype_object_bindings = self.local_prototype_object_bindings.clone();
        let saved_local_arguments_bindings = self.local_arguments_bindings.clone();
        let saved_direct_arguments_aliases = self.direct_arguments_aliases.clone();
        let saved_local_descriptor_bindings = self.local_descriptor_bindings.clone();
        let saved_eval_lexical_initialized_locals = self.eval_lexical_initialized_locals.clone();
        let saved_active_eval_lexical_scopes = self.active_eval_lexical_scopes.clone();
        let saved_active_eval_lexical_binding_counts =
            self.active_eval_lexical_binding_counts.clone();
        let saved_active_scoped_lexical_bindings = self.active_scoped_lexical_bindings.clone();
        let saved_with_scopes = self.with_scopes.clone();
        let saved_capture_slot_source_bindings = self.capture_slot_source_bindings.clone();
        let saved_last_bound_user_function_call = self.last_bound_user_function_call.clone();

        let result = callback(self);

        self.local_kinds = saved_local_kinds;
        self.local_value_bindings = saved_local_value_bindings;
        self.local_function_bindings = saved_local_function_bindings;
        self.local_specialized_function_values = saved_local_specialized_function_values;
        self.local_proxy_bindings = saved_local_proxy_bindings;
        self.member_function_bindings = saved_member_function_bindings;
        self.member_function_capture_slots = saved_member_function_capture_slots;
        self.member_getter_bindings = saved_member_getter_bindings;
        self.member_setter_bindings = saved_member_setter_bindings;
        self.local_array_bindings = saved_local_array_bindings;
        self.local_resizable_array_buffer_bindings = saved_local_resizable_array_buffer_bindings;
        self.local_typed_array_view_bindings = saved_local_typed_array_view_bindings;
        self.runtime_typed_array_oob_locals = saved_runtime_typed_array_oob_locals;
        self.tracked_array_function_values = saved_tracked_array_function_values;
        self.runtime_array_slots = saved_runtime_array_slots;
        self.local_array_iterator_bindings = saved_local_array_iterator_bindings;
        self.local_iterator_step_bindings = saved_local_iterator_step_bindings;
        self.runtime_array_length_locals = saved_runtime_array_length_locals;
        self.local_object_bindings = saved_local_object_bindings;
        self.local_prototype_object_bindings = saved_local_prototype_object_bindings;
        self.local_arguments_bindings = saved_local_arguments_bindings;
        self.direct_arguments_aliases = saved_direct_arguments_aliases;
        self.local_descriptor_bindings = saved_local_descriptor_bindings;
        self.eval_lexical_initialized_locals = saved_eval_lexical_initialized_locals;
        self.active_eval_lexical_scopes = saved_active_eval_lexical_scopes;
        self.active_eval_lexical_binding_counts = saved_active_eval_lexical_binding_counts;
        self.active_scoped_lexical_bindings = saved_active_scoped_lexical_bindings;
        self.with_scopes = saved_with_scopes;
        self.capture_slot_source_bindings = saved_capture_slot_source_bindings;
        self.last_bound_user_function_call = saved_last_bound_user_function_call;

        result
    }

    pub(in crate::backend::direct_wasm) fn with_narrowed_local_binding_metadata<T>(
        &mut self,
        name: &str,
        expression: &Expression,
        callback: impl FnOnce(&mut Self) -> DirectResult<T>,
    ) -> DirectResult<T> {
        let saved_value = self.local_value_bindings.get(name).cloned();
        let saved_array = self.local_array_bindings.get(name).cloned();
        let saved_object = self.local_object_bindings.get(name).cloned();
        let saved_kind = self.local_kinds.get(name).copied();

        self.local_value_bindings
            .insert(name.to_string(), expression.clone());
        if let Some(array_binding) = self.resolve_array_binding_from_expression(expression) {
            self.local_array_bindings
                .insert(name.to_string(), array_binding);
        } else {
            self.local_array_bindings.remove(name);
        }
        if let Some(object_binding) = self.resolve_object_binding_from_expression(expression) {
            self.local_object_bindings
                .insert(name.to_string(), object_binding);
        } else {
            self.local_object_bindings.remove(name);
        }
        if let Some(kind) = self.infer_value_kind(expression) {
            self.local_kinds.insert(name.to_string(), kind);
        } else {
            self.local_kinds.remove(name);
        }

        let result = callback(self);

        if let Some(value) = saved_value {
            self.local_value_bindings.insert(name.to_string(), value);
        } else {
            self.local_value_bindings.remove(name);
        }
        if let Some(array_binding) = saved_array {
            self.local_array_bindings
                .insert(name.to_string(), array_binding);
        } else {
            self.local_array_bindings.remove(name);
        }
        if let Some(object_binding) = saved_object {
            self.local_object_bindings
                .insert(name.to_string(), object_binding);
        } else {
            self.local_object_bindings.remove(name);
        }
        if let Some(kind) = saved_kind {
            self.local_kinds.insert(name.to_string(), kind);
        } else {
            self.local_kinds.remove(name);
        }

        result
    }

    pub(in crate::backend::direct_wasm) fn invalidate_static_binding_metadata_for_names(
        &mut self,
        names: &HashSet<String>,
    ) {
        for name in names {
            self.local_value_bindings.remove(name);
            self.local_array_bindings.remove(name);
            self.local_object_bindings.remove(name);
            self.local_function_bindings.remove(name);
            self.local_kinds.remove(name);
            self.local_arguments_bindings.remove(name);
            self.local_descriptor_bindings.remove(name);
            self.local_proxy_bindings.remove(name);
            self.local_prototype_object_bindings.remove(name);
            self.local_specialized_function_values.remove(name);

            self.module.global_value_bindings.remove(name);
            self.module.global_array_bindings.remove(name);
            self.module.global_object_bindings.remove(name);
            self.module.global_function_bindings.remove(name);
            self.module.global_kinds.remove(name);
            self.module.global_arguments_bindings.remove(name);
            self.module.global_proxy_bindings.remove(name);
            self.module.global_prototype_object_bindings.remove(name);
            self.module.global_property_descriptors.remove(name);
            self.module.global_specialized_function_values.remove(name);
        }
    }

    pub(in crate::backend::direct_wasm) fn invalidate_static_binding_metadata_for_names_with_preserved_kinds(
        &mut self,
        names: &HashSet<String>,
        preserved_kinds: &HashMap<String, StaticValueKind>,
    ) {
        self.invalidate_static_binding_metadata_for_names(names);
        for (name, kind) in preserved_kinds {
            if let Some((resolved_name, _)) = self.resolve_current_local_binding(name) {
                self.local_kinds.insert(resolved_name, *kind);
            } else if self.locals.contains_key(name)
                || self.parameter_scope_arguments_local_for(name).is_some()
            {
                self.local_kinds.insert(name.clone(), *kind);
            } else if let Some(hidden_name) = self.resolve_user_function_capture_hidden_name(name) {
                self.module.global_kinds.insert(hidden_name, *kind);
            } else if self.binding_name_is_global(name)
                || self.module.global_bindings.contains_key(name)
            {
                self.module.global_kinds.insert(name.clone(), *kind);
            } else {
                self.local_kinds.insert(name.clone(), *kind);
            }
        }
    }

    fn current_binding_kind_for_preservation(&self, name: &str) -> Option<StaticValueKind> {
        self.resolve_current_local_binding(name)
            .and_then(|(resolved_name, _)| self.local_kinds.get(&resolved_name).copied())
            .or_else(|| self.local_kinds.get(name).copied())
            .or_else(|| {
                self.resolve_user_function_capture_hidden_name(name)
                    .and_then(|hidden_name| self.module.global_kinds.get(&hidden_name).copied())
            })
            .or_else(|| self.module.global_kinds.get(name).copied())
            .filter(|kind| *kind != StaticValueKind::Unknown)
    }

    fn merge_preserved_binding_kind(
        &self,
        invalidated_bindings: &HashSet<String>,
        preserved_kinds: &mut HashMap<String, StaticValueKind>,
        blocked_bindings: &mut HashSet<String>,
        name: &str,
        candidate: Option<StaticValueKind>,
    ) {
        if !invalidated_bindings.contains(name) || blocked_bindings.contains(name) {
            return;
        }
        let Some(candidate) = candidate.filter(|kind| *kind != StaticValueKind::Unknown) else {
            preserved_kinds.remove(name);
            blocked_bindings.insert(name.to_string());
            return;
        };
        match preserved_kinds.get(name).copied() {
            Some(existing_kind) if existing_kind != candidate => {
                preserved_kinds.remove(name);
                blocked_bindings.insert(name.to_string());
            }
            Some(_) => {}
            None => {
                preserved_kinds.insert(name.to_string(), candidate);
            }
        }
    }

    fn collect_preserved_binding_kinds_from_expression(
        &self,
        invalidated_bindings: &HashSet<String>,
        preserved_kinds: &mut HashMap<String, StaticValueKind>,
        blocked_bindings: &mut HashSet<String>,
        expression: &Expression,
    ) {
        match expression {
            Expression::Update { name, .. } => {
                self.merge_preserved_binding_kind(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    name,
                    Some(StaticValueKind::Number),
                );
            }
            Expression::Assign { name, value } => {
                self.merge_preserved_binding_kind(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    name,
                    self.infer_value_kind(value),
                );
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    value,
                );
            }
            Expression::AssignMember {
                object,
                property,
                value,
            } => {
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    object,
                );
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    property,
                );
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    value,
                );
            }
            Expression::AssignSuperMember { property, value } => {
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    property,
                );
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    value,
                );
            }
            Expression::Member { object, property } => {
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    object,
                );
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    property,
                );
            }
            Expression::SuperMember { property } => {
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    property,
                );
            }
            Expression::Await(value)
            | Expression::EnumerateKeys(value)
            | Expression::GetIterator(value)
            | Expression::IteratorClose(value)
            | Expression::Unary {
                expression: value, ..
            } => {
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    value,
                );
            }
            Expression::Binary { left, right, .. } => {
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    left,
                );
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    right,
                );
            }
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    condition,
                );
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    then_expression,
                );
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    else_expression,
                );
            }
            Expression::Sequence(expressions) => {
                for expression in expressions {
                    self.collect_preserved_binding_kinds_from_expression(
                        invalidated_bindings,
                        preserved_kinds,
                        blocked_bindings,
                        expression,
                    );
                }
            }
            Expression::Call { callee, arguments }
            | Expression::SuperCall { callee, arguments }
            | Expression::New { callee, arguments } => {
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    callee,
                );
                for argument in arguments {
                    match argument {
                        CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                            self.collect_preserved_binding_kinds_from_expression(
                                invalidated_bindings,
                                preserved_kinds,
                                blocked_bindings,
                                expression,
                            );
                        }
                    }
                }
            }
            Expression::Array(elements) => {
                for element in elements {
                    match element {
                        crate::ir::hir::ArrayElement::Expression(expression)
                        | crate::ir::hir::ArrayElement::Spread(expression) => {
                            self.collect_preserved_binding_kinds_from_expression(
                                invalidated_bindings,
                                preserved_kinds,
                                blocked_bindings,
                                expression,
                            );
                        }
                    }
                }
            }
            Expression::Object(entries) => {
                for entry in entries {
                    match entry {
                        crate::ir::hir::ObjectEntry::Data { key, value } => {
                            self.collect_preserved_binding_kinds_from_expression(
                                invalidated_bindings,
                                preserved_kinds,
                                blocked_bindings,
                                key,
                            );
                            self.collect_preserved_binding_kinds_from_expression(
                                invalidated_bindings,
                                preserved_kinds,
                                blocked_bindings,
                                value,
                            );
                        }
                        crate::ir::hir::ObjectEntry::Getter { key, getter } => {
                            self.collect_preserved_binding_kinds_from_expression(
                                invalidated_bindings,
                                preserved_kinds,
                                blocked_bindings,
                                key,
                            );
                            self.collect_preserved_binding_kinds_from_expression(
                                invalidated_bindings,
                                preserved_kinds,
                                blocked_bindings,
                                getter,
                            );
                        }
                        crate::ir::hir::ObjectEntry::Setter { key, setter } => {
                            self.collect_preserved_binding_kinds_from_expression(
                                invalidated_bindings,
                                preserved_kinds,
                                blocked_bindings,
                                key,
                            );
                            self.collect_preserved_binding_kinds_from_expression(
                                invalidated_bindings,
                                preserved_kinds,
                                blocked_bindings,
                                setter,
                            );
                        }
                        crate::ir::hir::ObjectEntry::Spread(expression) => {
                            self.collect_preserved_binding_kinds_from_expression(
                                invalidated_bindings,
                                preserved_kinds,
                                blocked_bindings,
                                expression,
                            );
                        }
                    }
                }
            }
            Expression::Identifier(_)
            | Expression::Number(_)
            | Expression::BigInt(_)
            | Expression::String(_)
            | Expression::Bool(_)
            | Expression::Null
            | Expression::Undefined
            | Expression::This
            | Expression::NewTarget
            | Expression::Sent => {}
        }
    }

    fn collect_preserved_binding_kinds_from_statement(
        &self,
        invalidated_bindings: &HashSet<String>,
        preserved_kinds: &mut HashMap<String, StaticValueKind>,
        blocked_bindings: &mut HashSet<String>,
        statement: &Statement,
    ) {
        match statement {
            Statement::Block { body } | Statement::Labeled { body, .. } => {
                for statement in body {
                    self.collect_preserved_binding_kinds_from_statement(
                        invalidated_bindings,
                        preserved_kinds,
                        blocked_bindings,
                        statement,
                    );
                }
            }
            Statement::Var { name, value } | Statement::Let { name, value, .. } => {
                self.merge_preserved_binding_kind(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    name,
                    self.infer_value_kind(value),
                );
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    value,
                );
            }
            Statement::Assign { name, value } => {
                self.merge_preserved_binding_kind(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    name,
                    self.infer_value_kind(value),
                );
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    value,
                );
            }
            Statement::AssignMember {
                object,
                property,
                value,
            } => {
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    object,
                );
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    property,
                );
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    value,
                );
            }
            Statement::Expression(expression)
            | Statement::Throw(expression)
            | Statement::Return(expression)
            | Statement::Yield { value: expression }
            | Statement::YieldDelegate { value: expression } => {
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    expression,
                );
            }
            Statement::Print { values } => {
                for value in values {
                    self.collect_preserved_binding_kinds_from_expression(
                        invalidated_bindings,
                        preserved_kinds,
                        blocked_bindings,
                        value,
                    );
                }
            }
            Statement::With { object, body } => {
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    object,
                );
                for statement in body {
                    self.collect_preserved_binding_kinds_from_statement(
                        invalidated_bindings,
                        preserved_kinds,
                        blocked_bindings,
                        statement,
                    );
                }
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    condition,
                );
                for statement in then_branch {
                    self.collect_preserved_binding_kinds_from_statement(
                        invalidated_bindings,
                        preserved_kinds,
                        blocked_bindings,
                        statement,
                    );
                }
                for statement in else_branch {
                    self.collect_preserved_binding_kinds_from_statement(
                        invalidated_bindings,
                        preserved_kinds,
                        blocked_bindings,
                        statement,
                    );
                }
            }
            Statement::Try {
                body,
                catch_setup,
                catch_body,
                ..
            } => {
                for statement in body {
                    self.collect_preserved_binding_kinds_from_statement(
                        invalidated_bindings,
                        preserved_kinds,
                        blocked_bindings,
                        statement,
                    );
                }
                for statement in catch_setup {
                    self.collect_preserved_binding_kinds_from_statement(
                        invalidated_bindings,
                        preserved_kinds,
                        blocked_bindings,
                        statement,
                    );
                }
                for statement in catch_body {
                    self.collect_preserved_binding_kinds_from_statement(
                        invalidated_bindings,
                        preserved_kinds,
                        blocked_bindings,
                        statement,
                    );
                }
            }
            Statement::Switch {
                discriminant,
                cases,
                ..
            } => {
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    discriminant,
                );
                for case in cases {
                    if let Some(test) = &case.test {
                        self.collect_preserved_binding_kinds_from_expression(
                            invalidated_bindings,
                            preserved_kinds,
                            blocked_bindings,
                            test,
                        );
                    }
                    for statement in &case.body {
                        self.collect_preserved_binding_kinds_from_statement(
                            invalidated_bindings,
                            preserved_kinds,
                            blocked_bindings,
                            statement,
                        );
                    }
                }
            }
            Statement::For {
                init,
                condition,
                update,
                break_hook,
                body,
                ..
            } => {
                for statement in init {
                    self.collect_preserved_binding_kinds_from_statement(
                        invalidated_bindings,
                        preserved_kinds,
                        blocked_bindings,
                        statement,
                    );
                }
                if let Some(condition) = condition {
                    self.collect_preserved_binding_kinds_from_expression(
                        invalidated_bindings,
                        preserved_kinds,
                        blocked_bindings,
                        condition,
                    );
                }
                if let Some(update) = update {
                    self.collect_preserved_binding_kinds_from_expression(
                        invalidated_bindings,
                        preserved_kinds,
                        blocked_bindings,
                        update,
                    );
                }
                if let Some(break_hook) = break_hook {
                    self.collect_preserved_binding_kinds_from_expression(
                        invalidated_bindings,
                        preserved_kinds,
                        blocked_bindings,
                        break_hook,
                    );
                }
                for statement in body {
                    self.collect_preserved_binding_kinds_from_statement(
                        invalidated_bindings,
                        preserved_kinds,
                        blocked_bindings,
                        statement,
                    );
                }
            }
            Statement::While {
                condition,
                break_hook,
                body,
                ..
            }
            | Statement::DoWhile {
                condition,
                break_hook,
                body,
                ..
            } => {
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    condition,
                );
                if let Some(break_hook) = break_hook {
                    self.collect_preserved_binding_kinds_from_expression(
                        invalidated_bindings,
                        preserved_kinds,
                        blocked_bindings,
                        break_hook,
                    );
                }
                for statement in body {
                    self.collect_preserved_binding_kinds_from_statement(
                        invalidated_bindings,
                        preserved_kinds,
                        blocked_bindings,
                        statement,
                    );
                }
            }
            Statement::Break { .. } | Statement::Continue { .. } => {}
        }
    }

    fn preserved_binding_kinds_for_loop(
        &self,
        invalidated_bindings: &HashSet<String>,
        condition: &Expression,
        break_hook: Option<&Expression>,
        body: &[Statement],
        update: Option<&Expression>,
    ) -> HashMap<String, StaticValueKind> {
        let mut preserved_kinds = HashMap::new();
        for name in invalidated_bindings {
            if let Some(kind) = self.current_binding_kind_for_preservation(name) {
                preserved_kinds.insert(name.clone(), kind);
            }
        }
        let mut blocked_bindings = HashSet::new();
        self.collect_preserved_binding_kinds_from_expression(
            invalidated_bindings,
            &mut preserved_kinds,
            &mut blocked_bindings,
            condition,
        );
        if let Some(update) = update {
            self.collect_preserved_binding_kinds_from_expression(
                invalidated_bindings,
                &mut preserved_kinds,
                &mut blocked_bindings,
                update,
            );
        }
        if let Some(break_hook) = break_hook {
            self.collect_preserved_binding_kinds_from_expression(
                invalidated_bindings,
                &mut preserved_kinds,
                &mut blocked_bindings,
                break_hook,
            );
        }
        for statement in body {
            self.collect_preserved_binding_kinds_from_statement(
                invalidated_bindings,
                &mut preserved_kinds,
                &mut blocked_bindings,
                statement,
            );
        }
        preserved_kinds
    }

    pub(in crate::backend::direct_wasm) fn emit_do_while(
        &mut self,
        condition: &Expression,
        break_hook: Option<&Expression>,
        labels: &[String],
        body: &[Statement],
    ) -> DirectResult<()> {
        let invalidated_bindings =
            collect_loop_assigned_binding_names(condition, break_hook, body, None, None);
        let preserved_kinds = self.preserved_binding_kinds_for_loop(
            &invalidated_bindings,
            condition,
            break_hook,
            body,
            None,
        );
        self.invalidate_static_binding_metadata_for_names_with_preserved_kinds(
            &invalidated_bindings,
            &preserved_kinds,
        );
        self.instructions.push(0x02);
        self.instructions.push(EMPTY_BLOCK_TYPE);
        let break_target = self.push_control_frame();

        self.instructions.push(0x03);
        self.instructions.push(EMPTY_BLOCK_TYPE);
        let loop_target = self.push_control_frame();

        self.instructions.push(0x02);
        self.instructions.push(EMPTY_BLOCK_TYPE);
        let continue_target = self.push_control_frame();
        self.loop_stack.push(LoopContext {
            break_target,
            continue_target,
            labels: labels.to_vec(),
            assigned_bindings: invalidated_bindings.clone(),
        });
        self.break_stack.push(BreakContext {
            break_target,
            labels: labels.to_vec(),
            break_hook: break_hook.cloned(),
        });

        self.emit_statements(body)?;
        self.instructions.push(0x0b);
        self.pop_control_frame();

        self.emit_numeric_expression(condition)?;
        self.instructions.push(0x45);
        self.push_br_if(self.relative_depth(break_target));
        self.push_br(self.relative_depth(loop_target));

        self.loop_stack.pop();
        self.break_stack.pop();
        self.instructions.push(0x0b);
        self.pop_control_frame();
        self.instructions.push(0x0b);
        self.pop_control_frame();
        self.invalidate_static_binding_metadata_for_names_with_preserved_kinds(
            &invalidated_bindings,
            &preserved_kinds,
        );
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_for(
        &mut self,
        labels: &[String],
        init: &[Statement],
        per_iteration_bindings: &[String],
        condition: Option<&Expression>,
        update: Option<&Expression>,
        break_hook: Option<&Expression>,
        body: &[Statement],
    ) -> DirectResult<()> {
        let invalidated_bindings =
            collect_loop_assigned_binding_names_from_for(init, condition, update, break_hook, body);
        self.with_active_eval_lexical_scope(per_iteration_bindings.to_vec(), |compiler| {
            compiler.emit_statements(init)?;
            let fallback_condition = Expression::Bool(true);
            let preserved_kinds = compiler.preserved_binding_kinds_for_loop(
                &invalidated_bindings,
                condition.unwrap_or(&fallback_condition),
                break_hook,
                body,
                update,
            );
            compiler.invalidate_static_binding_metadata_for_names_with_preserved_kinds(
                &invalidated_bindings,
                &preserved_kinds,
            );

            compiler.instructions.push(0x02);
            compiler.instructions.push(EMPTY_BLOCK_TYPE);
            let break_target = compiler.push_control_frame();

            compiler.instructions.push(0x03);
            compiler.instructions.push(EMPTY_BLOCK_TYPE);
            let loop_target = compiler.push_control_frame();

            if let Some(condition) = condition {
                compiler.emit_numeric_expression(condition)?;
                compiler.instructions.push(0x45);
                compiler.push_br_if(compiler.relative_depth(break_target));
            }

            compiler.instructions.push(0x02);
            compiler.instructions.push(EMPTY_BLOCK_TYPE);
            let continue_target = compiler.push_control_frame();
            compiler.loop_stack.push(LoopContext {
                break_target,
                continue_target,
                labels: labels.to_vec(),
                assigned_bindings: invalidated_bindings.clone(),
            });
            compiler.break_stack.push(BreakContext {
                break_target,
                labels: labels.to_vec(),
                break_hook: break_hook.cloned(),
            });

            compiler.emit_statements(body)?;
            compiler.instructions.push(0x0b);
            compiler.pop_control_frame();

            if let Some(update) = update {
                compiler.emit_numeric_expression(update)?;
                compiler.instructions.push(0x1a);
            }
            compiler.push_br(compiler.relative_depth(loop_target));

            compiler.loop_stack.pop();
            compiler.break_stack.pop();
            compiler.instructions.push(0x0b);
            compiler.pop_control_frame();
            compiler.instructions.push(0x0b);
            compiler.pop_control_frame();
            compiler.invalidate_static_binding_metadata_for_names_with_preserved_kinds(
                &invalidated_bindings,
                &preserved_kinds,
            );
            Ok(())
        })
    }

    pub(in crate::backend::direct_wasm) fn emit_switch_case(
        &mut self,
        case: &crate::ir::hir::SwitchCase,
        active_local: u32,
        discriminant_local: u32,
    ) -> DirectResult<()> {
        if let Some(test) = &case.test {
            self.push_local_get(active_local);
            self.push_i32_const(1);
            self.instructions.push(0x46);

            self.push_local_get(active_local);
            self.instructions.push(0x45);
            self.push_local_get(discriminant_local);
            self.emit_numeric_expression(test)?;
            self.instructions.push(0x46);
            self.instructions.push(0x71);
            self.instructions.push(0x72);
        } else {
            self.push_local_get(active_local);
            self.instructions.push(0x45);
        }

        self.instructions.push(0x04);
        self.instructions.push(EMPTY_BLOCK_TYPE);
        self.push_control_frame();

        self.push_i32_const(1);
        self.push_local_set(active_local);
        for case_statement in &case.body {
            self.emit_statement(case_statement)?;
        }

        self.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(())
    }
}
