use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_inline_user_function_summary_with_arguments(
        &mut self,
        user_function: &UserFunction,
        arguments: &[Expression],
    ) -> DirectResult<bool> {
        let call_arguments = arguments
            .iter()
            .cloned()
            .map(CallArgument::Expression)
            .collect::<Vec<_>>();

        if let Some(summary) = user_function.inline_summary.as_ref() {
            self.emit_inline_summary_with_call_arguments(user_function, summary, &call_arguments)?;
            return Ok(true);
        }

        let Some(function) = self
            .resolve_registered_function_declaration(&user_function.name)
            .cloned()
        else {
            return Ok(false);
        };
        let Some((terminal_statement, effect_statements)) = function.body.split_last() else {
            return Ok(false);
        };

        let previous_strict_mode = self.strict_mode;
        let previous_user_function_name = self.current_user_function_name.clone();
        self.strict_mode = user_function.strict;
        self.current_user_function_name = Some(user_function.name.clone());
        for statement in effect_statements {
            match statement {
                Statement::Assign { name, value } => {
                    self.emit_statement(&Statement::Assign {
                        name: name.clone(),
                        value: self.substitute_user_function_argument_bindings(
                            value,
                            user_function,
                            &call_arguments,
                        ),
                    })?;
                }
                Statement::Expression(Expression::Update { name, op, prefix }) => {
                    self.emit_numeric_expression(&Expression::Update {
                        name: name.clone(),
                        op: *op,
                        prefix: *prefix,
                    })?;
                    self.instructions.push(0x1a);
                }
                Statement::Expression(expression) => {
                    let substituted = self.substitute_user_function_argument_bindings(
                        expression,
                        user_function,
                        &call_arguments,
                    );
                    self.emit_numeric_expression(&substituted)?;
                    self.instructions.push(0x1a);
                }
                Statement::Block { body } if body.is_empty() => {}
                _ => {
                    self.strict_mode = previous_strict_mode;
                    self.current_user_function_name = previous_user_function_name;
                    return Ok(false);
                }
            }
        }
        match terminal_statement {
            Statement::Return(return_value) => {
                let substituted = self.substitute_user_function_argument_bindings(
                    return_value,
                    user_function,
                    &call_arguments,
                );
                self.emit_numeric_expression(&substituted)?;
            }
            Statement::Throw(throw_value) => {
                let substituted = self.substitute_user_function_argument_bindings(
                    throw_value,
                    user_function,
                    &call_arguments,
                );
                self.emit_statement(&Statement::Throw(substituted))?;
                self.push_i32_const(JS_UNDEFINED_TAG);
            }
            _ => {
                self.strict_mode = previous_strict_mode;
                self.current_user_function_name = previous_user_function_name;
                return Ok(false);
            }
        }
        self.strict_mode = previous_strict_mode;
        self.current_user_function_name = previous_user_function_name;
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_inline_user_function_summary_with_argument_locals(
        &mut self,
        user_function: &UserFunction,
        argument_locals: &[u32],
        argument_count: usize,
    ) -> DirectResult<bool> {
        let Some(summary) = user_function.inline_summary.as_ref() else {
            return Ok(false);
        };
        if !user_function.extra_argument_indices.is_empty()
            || user_function.has_parameter_defaults()
            || (inline_summary_mentions_call_frame_state(summary) && !user_function.lexical_this)
            || argument_locals.len() < argument_count
        {
            return Ok(false);
        }

        let mut argument_names = Vec::with_capacity(argument_count);
        for (index, argument_local) in argument_locals
            .iter()
            .copied()
            .take(argument_count)
            .enumerate()
        {
            let hidden_name = self.allocate_named_hidden_local(
                &format!("inline_arg_{index}"),
                StaticValueKind::Unknown,
            );
            let hidden_local = self
                .locals
                .get(&hidden_name)
                .copied()
                .expect("hidden inline argument local should exist");
            self.push_local_get(argument_local);
            self.push_local_set(hidden_local);
            argument_names.push(hidden_name);
        }

        let call_arguments = argument_names
            .into_iter()
            .map(Expression::Identifier)
            .map(CallArgument::Expression)
            .collect::<Vec<_>>();
        self.emit_inline_summary_with_call_arguments(user_function, summary, &call_arguments)?;
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_inline_summary_with_call_arguments(
        &mut self,
        user_function: &UserFunction,
        summary: &InlineFunctionSummary,
        call_arguments: &[CallArgument],
    ) -> DirectResult<()> {
        let previous_strict_mode = self.strict_mode;
        let previous_user_function_name = self.current_user_function_name.clone();
        self.strict_mode = user_function.strict;
        self.current_user_function_name = Some(user_function.name.clone());
        for effect in &summary.effects {
            match effect {
                InlineFunctionEffect::Assign { name, value } => {
                    self.emit_statement(&Statement::Assign {
                        name: name.clone(),
                        value: self.substitute_user_function_argument_bindings(
                            value,
                            user_function,
                            call_arguments,
                        ),
                    })?;
                }
                InlineFunctionEffect::Update { name, op, prefix } => {
                    self.emit_numeric_expression(&Expression::Update {
                        name: name.clone(),
                        op: *op,
                        prefix: *prefix,
                    })?;
                    self.instructions.push(0x1a);
                }
                InlineFunctionEffect::Expression(expression) => {
                    let substituted = self.substitute_user_function_argument_bindings(
                        expression,
                        user_function,
                        call_arguments,
                    );
                    self.emit_numeric_expression(&substituted)?;
                    self.instructions.push(0x1a);
                }
            }
        }
        if let Some(return_value) = summary.return_value.as_ref() {
            let substituted = self.substitute_user_function_argument_bindings(
                return_value,
                user_function,
                call_arguments,
            );
            self.emit_numeric_expression(&substituted)?;
        } else {
            self.push_i32_const(JS_UNDEFINED_TAG);
        }
        self.strict_mode = previous_strict_mode;
        self.current_user_function_name = previous_user_function_name;
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_specialized_function_value_call(
        &mut self,
        specialized: &SpecializedFunctionValue,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        let LocalFunctionBinding::User(function_name) = &specialized.binding else {
            return Ok(false);
        };
        let Some(user_function) = self.module.user_function_map.get(function_name).cloned() else {
            return Ok(false);
        };
        if user_function.is_async()
            || user_function.is_generator()
            || user_function.has_parameter_defaults()
        {
            return Ok(false);
        }
        self.emit_inline_summary_with_call_arguments(
            &user_function,
            &specialized.summary,
            arguments,
        )?;
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn inline_safe_argument_expression(
        &self,
        expression: &Expression,
    ) -> bool {
        let materialized = self.materialize_static_expression(expression);
        matches!(
            materialized,
            Expression::Number(_)
                | Expression::BigInt(_)
                | Expression::String(_)
                | Expression::Bool(_)
                | Expression::Null
                | Expression::Undefined
                | Expression::Array(_)
        ) || matches!(materialized, Expression::Object(ref entries)
            if entries.iter().all(|entry| matches!(entry, ObjectEntry::Data { .. })))
            || matches!(materialized, Expression::Identifier(_))
                && (self
                    .resolve_object_binding_from_expression(expression)
                    .is_some()
                    || self
                        .resolve_array_binding_from_expression(expression)
                        .is_some())
    }

    pub(in crate::backend::direct_wasm) fn can_inline_user_function_call(
        &self,
        user_function: &UserFunction,
        arguments: &[Expression],
    ) -> bool {
        arguments
            .iter()
            .all(|argument| self.inline_safe_argument_expression(argument))
            && !arguments
                .iter()
                .any(|argument| self.inline_argument_mentions_shadowed_implicit_global(argument))
            && !user_function.is_async()
            && !user_function.is_generator()
            && !self
                .module
                .user_function_capture_bindings
                .contains_key(&user_function.name)
            && !self.user_function_references_captured_user_function(user_function)
            && user_function.extra_argument_indices.is_empty()
            && !user_function.has_parameter_defaults()
            && (user_function
                .inline_summary
                .as_ref()
                .is_some_and(|summary| {
                    !inline_summary_mentions_assertion_builtin(summary)
                        && (user_function.lexical_this
                            || !inline_summary_mentions_call_frame_state(summary))
                })
                || self.user_function_has_inlineable_terminal_body(user_function))
    }

    pub(in crate::backend::direct_wasm) fn can_inline_user_function_call_with_explicit_call_frame(
        &self,
        user_function: &UserFunction,
        arguments: &[Expression],
        this_expression: &Expression,
    ) -> bool {
        self.inline_safe_argument_expression(this_expression)
            && !self.inline_argument_mentions_shadowed_implicit_global(this_expression)
            && arguments
                .iter()
                .all(|argument| self.inline_safe_argument_expression(argument))
            && !arguments
                .iter()
                .any(|argument| self.inline_argument_mentions_shadowed_implicit_global(argument))
            && !user_function.is_async()
            && !user_function.is_generator()
            && !self
                .module
                .user_function_capture_bindings
                .contains_key(&user_function.name)
            && !self.user_function_references_captured_user_function(user_function)
            && user_function.extra_argument_indices.is_empty()
            && !user_function.has_parameter_defaults()
            && (user_function
                .inline_summary
                .as_ref()
                .is_some_and(|summary| {
                    !inline_summary_mentions_assertion_builtin(summary)
                        && !inline_summary_mentions_unsupported_explicit_call_frame_state(summary)
                })
                || self
                    .user_function_has_explicit_call_frame_inlineable_terminal_body(user_function))
    }

    pub(in crate::backend::direct_wasm) fn user_function_has_inlineable_terminal_body(
        &self,
        user_function: &UserFunction,
    ) -> bool {
        let Some(function) = self.resolve_registered_function_declaration(&user_function.name)
        else {
            return false;
        };
        let Some((terminal_statement, effect_statements)) = function.body.split_last() else {
            return false;
        };
        for statement in effect_statements {
            match statement {
                Statement::Assign { value, .. } => {
                    if !user_function.lexical_this && expression_mentions_call_frame_state(value) {
                        return false;
                    }
                }
                Statement::Expression(Expression::Update { .. }) => {}
                Statement::Expression(expression) => {
                    if !user_function.lexical_this
                        && expression_mentions_call_frame_state(expression)
                    {
                        return false;
                    }
                }
                Statement::Block { body } if body.is_empty() => {}
                _ => return false,
            }
        }
        match terminal_statement {
            Statement::Return(expression) | Statement::Throw(expression) => {
                user_function.lexical_this || !expression_mentions_call_frame_state(expression)
            }
            _ => false,
        }
    }

    pub(in crate::backend::direct_wasm) fn user_function_has_explicit_call_frame_inlineable_terminal_body(
        &self,
        user_function: &UserFunction,
    ) -> bool {
        let Some(function) = self.resolve_registered_function_declaration(&user_function.name)
        else {
            return false;
        };
        let Some((terminal_statement, effect_statements)) = function.body.split_last() else {
            return false;
        };
        for statement in effect_statements {
            match statement {
                Statement::Assign { value, .. } => {
                    if expression_mentions_unsupported_explicit_call_frame_state(value) {
                        return false;
                    }
                }
                Statement::Expression(Expression::Update { .. }) => {}
                Statement::Expression(expression) => {
                    if expression_mentions_unsupported_explicit_call_frame_state(expression) {
                        return false;
                    }
                }
                Statement::Block { body } if body.is_empty() => {}
                _ => return false,
            }
        }
        match terminal_statement {
            Statement::Return(expression) | Statement::Throw(expression) => {
                !expression_mentions_unsupported_explicit_call_frame_state(expression)
            }
            _ => false,
        }
    }

    pub(in crate::backend::direct_wasm) fn inline_argument_mentions_shadowed_implicit_global(
        &self,
        expression: &Expression,
    ) -> bool {
        match expression {
            Expression::Identifier(name) => {
                self.resolve_current_local_binding(name).is_some()
                    && self.module.implicit_global_bindings.contains_key(name)
            }
            Expression::Member { object, property } => {
                self.inline_argument_mentions_shadowed_implicit_global(object)
                    || self.inline_argument_mentions_shadowed_implicit_global(property)
            }
            Expression::SuperMember { property } => {
                self.inline_argument_mentions_shadowed_implicit_global(property)
            }
            Expression::Assign { value, .. } => {
                self.inline_argument_mentions_shadowed_implicit_global(value)
            }
            Expression::AssignMember {
                object,
                property,
                value,
            } => {
                self.inline_argument_mentions_shadowed_implicit_global(object)
                    || self.inline_argument_mentions_shadowed_implicit_global(property)
                    || self.inline_argument_mentions_shadowed_implicit_global(value)
            }
            Expression::AssignSuperMember { property, value } => {
                self.inline_argument_mentions_shadowed_implicit_global(property)
                    || self.inline_argument_mentions_shadowed_implicit_global(value)
            }
            Expression::Await(value)
            | Expression::EnumerateKeys(value)
            | Expression::GetIterator(value)
            | Expression::IteratorClose(value)
            | Expression::Unary {
                expression: value, ..
            } => self.inline_argument_mentions_shadowed_implicit_global(value),
            Expression::Binary { left, right, .. } => {
                self.inline_argument_mentions_shadowed_implicit_global(left)
                    || self.inline_argument_mentions_shadowed_implicit_global(right)
            }
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                self.inline_argument_mentions_shadowed_implicit_global(condition)
                    || self.inline_argument_mentions_shadowed_implicit_global(then_expression)
                    || self.inline_argument_mentions_shadowed_implicit_global(else_expression)
            }
            Expression::Sequence(expressions) => expressions.iter().any(|expression| {
                self.inline_argument_mentions_shadowed_implicit_global(expression)
            }),
            Expression::Array(elements) => elements.iter().any(|element| match element {
                ArrayElement::Expression(expression) | ArrayElement::Spread(expression) => {
                    self.inline_argument_mentions_shadowed_implicit_global(expression)
                }
            }),
            Expression::Call { callee, arguments }
            | Expression::SuperCall { callee, arguments } => {
                self.inline_argument_mentions_shadowed_implicit_global(callee)
                    || arguments.iter().any(|argument| match argument {
                        CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                            self.inline_argument_mentions_shadowed_implicit_global(expression)
                        }
                    })
            }
            Expression::New { callee, arguments } => {
                self.inline_argument_mentions_shadowed_implicit_global(callee)
                    || arguments.iter().any(|argument| match argument {
                        CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                            self.inline_argument_mentions_shadowed_implicit_global(expression)
                        }
                    })
            }
            Expression::Object(entries) => entries.iter().any(|entry| match entry {
                crate::ir::hir::ObjectEntry::Data { key, value } => {
                    self.inline_argument_mentions_shadowed_implicit_global(key)
                        || self.inline_argument_mentions_shadowed_implicit_global(value)
                }
                crate::ir::hir::ObjectEntry::Getter { key, getter } => {
                    self.inline_argument_mentions_shadowed_implicit_global(key)
                        || self.inline_argument_mentions_shadowed_implicit_global(getter)
                }
                crate::ir::hir::ObjectEntry::Setter { key, setter } => {
                    self.inline_argument_mentions_shadowed_implicit_global(key)
                        || self.inline_argument_mentions_shadowed_implicit_global(setter)
                }
                crate::ir::hir::ObjectEntry::Spread(expression) => {
                    self.inline_argument_mentions_shadowed_implicit_global(expression)
                }
            }),
            Expression::NewTarget
            | Expression::This
            | Expression::Number(_)
            | Expression::BigInt(_)
            | Expression::String(_)
            | Expression::Bool(_)
            | Expression::Null
            | Expression::Undefined
            | Expression::Sent
            | Expression::Update { .. } => false,
        }
    }

    pub(in crate::backend::direct_wasm) fn emit_delete_result_or_throw_if_strict(
        &mut self,
    ) -> DirectResult<()> {
        if !self.strict_mode {
            return Ok(());
        }
        let result_local = self.allocate_temp_local();
        self.push_local_set(result_local);
        self.push_local_get(result_local);
        self.instructions.push(0x45);
        self.instructions.push(0x04);
        self.instructions.push(EMPTY_BLOCK_TYPE);
        self.push_control_frame();
        self.emit_error_throw()?;
        self.instructions.push(0x0b);
        self.pop_control_frame();
        self.push_local_get(result_local);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn user_function_references_captured_user_function(
        &self,
        user_function: &UserFunction,
    ) -> bool {
        if self.module.user_function_capture_bindings.is_empty() {
            return false;
        }
        let captured_user_function_names = self
            .module
            .user_function_capture_bindings
            .keys()
            .cloned()
            .collect::<HashSet<_>>();
        self.module
            .registered_function_declarations
            .iter()
            .find(|function| function.name == user_function.name)
            .is_some_and(|function| {
                function.body.iter().any(|statement| {
                    statement_references_user_function(statement, &captured_user_function_names)
                })
            })
    }
}
