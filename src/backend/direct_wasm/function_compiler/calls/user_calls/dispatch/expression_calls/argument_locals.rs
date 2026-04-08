use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_user_function_call_with_new_target_and_this_expression_from_argument_locals(
        &mut self,
        user_function: &UserFunction,
        argument_locals: &[u32],
        argument_count: usize,
        new_target_value: i32,
        this_expression: &Expression,
    ) -> DirectResult<()> {
        let prepared_capture_bindings =
            self.prepare_user_function_capture_bindings(user_function)?;
        let synced_capture_source_bindings =
            self.synced_prepared_user_function_capture_source_bindings(&prepared_capture_bindings);
        let capture_snapshot =
            self.snapshot_user_function_capture_source_bindings(&prepared_capture_bindings);
        let static_this_expression = self.resolve_static_snapshot_this_expression(this_expression);
        let bound_argument_expressions = argument_locals
            .iter()
            .take(argument_count)
            .map(|argument_local| {
                self.state
                    .runtime
                    .locals
                    .iter()
                    .find_map(|(name, local)| {
                        (*local == *argument_local).then_some(
                            self.state
                                .speculation
                                .static_semantics
                                .local_value_binding(name)
                                .cloned()
                                .or_else(|| {
                                    self.resolve_object_binding_from_expression(
                                        &Expression::Identifier(name.clone()),
                                    )
                                    .map(|binding| object_binding_to_expression(&binding))
                                })
                                .or_else(|| {
                                    self.resolve_array_binding_from_expression(
                                        &Expression::Identifier(name.clone()),
                                    )
                                    .map(|binding| {
                                        Expression::Array(
                                            binding
                                                .values
                                                .into_iter()
                                                .map(|value| {
                                                    ArrayElement::Expression(
                                                        value.unwrap_or(Expression::Undefined),
                                                    )
                                                })
                                                .collect(),
                                        )
                                    })
                                })
                                .unwrap_or(Expression::Identifier(name.clone())),
                        )
                    })
                    .unwrap_or(Expression::Undefined)
            })
            .collect::<Vec<_>>();
        let static_result = if new_target_value == JS_UNDEFINED_TAG {
            self.resolve_bound_snapshot_user_function_result_with_arguments_and_this(
                &user_function.name,
                &capture_snapshot,
                &bound_argument_expressions,
                &static_this_expression,
            )
        } else {
            None
        };
        let updated_bindings = static_result
            .as_ref()
            .map(|(_, updated_bindings)| updated_bindings.clone());
        self.state
            .speculation
            .static_semantics
            .last_bound_user_function_call = Some(BoundUserFunctionCallSnapshot {
            function_name: user_function.name.clone(),
            source_expression: None,
            result_expression: static_result.as_ref().map(|(result, _)| result.clone()),
            updated_bindings: updated_bindings
                .clone()
                .unwrap_or_else(|| capture_snapshot.clone()),
        });
        let assigned_nonlocal_bindings =
            self.collect_user_function_assigned_nonlocal_bindings(user_function);
        let mut call_effect_nonlocal_bindings =
            self.collect_user_function_call_effect_nonlocal_bindings(user_function);
        call_effect_nonlocal_bindings.extend(
            self.collect_user_function_argument_call_effect_nonlocal_bindings(
                user_function,
                &bound_argument_expressions,
            ),
        );
        let assigned_nonlocal_binding_results = self
            .assigned_nonlocal_binding_results(&user_function.name)
            .cloned();
        let mut additional_call_effect_nonlocal_bindings = call_effect_nonlocal_bindings
            .iter()
            .filter(|name| !synced_capture_source_bindings.contains(*name))
            .cloned()
            .collect::<HashSet<_>>();
        additional_call_effect_nonlocal_bindings.extend(
            self.collect_snapshot_updated_nonlocal_bindings(
                user_function,
                updated_bindings.as_ref(),
            ),
        );
        let updated_nonlocal_bindings =
            self.collect_user_function_updated_nonlocal_bindings(user_function);

        let saved_new_target_local = if user_function.lexical_this {
            None
        } else {
            let saved_local = self.allocate_temp_local();
            self.push_global_get(CURRENT_NEW_TARGET_GLOBAL_INDEX);
            self.push_local_set(saved_local);
            self.push_i32_const(new_target_value);
            self.push_global_set(CURRENT_NEW_TARGET_GLOBAL_INDEX);
            Some(saved_local)
        };
        let saved_this_local = if user_function.lexical_this {
            None
        } else {
            let saved_local = self.allocate_temp_local();
            let this_local = self.allocate_temp_local();
            self.push_global_get(CURRENT_THIS_GLOBAL_INDEX);
            self.push_local_set(saved_local);
            self.emit_numeric_expression(this_expression)?;
            self.push_local_set(this_local);
            self.push_local_get(this_local);
            self.push_global_set(CURRENT_THIS_GLOBAL_INDEX);
            Some(saved_local)
        };

        self.emit_prepare_user_function_capture_globals(&user_function.name)?;

        let visible_param_count = user_function.visible_param_count() as usize;

        for argument_index in 0..visible_param_count {
            if let Some(argument_local) = argument_locals.get(argument_index).copied() {
                self.push_local_get(argument_local);
            } else {
                self.push_i32_const(JS_UNDEFINED_TAG);
            }
        }

        self.push_i32_const(argument_count as i32);

        for index in &user_function.extra_argument_indices {
            if let Some(argument_local) = argument_locals.get(*index as usize).copied() {
                self.push_local_get(argument_local);
            } else {
                self.push_i32_const(JS_UNDEFINED_TAG);
            }
        }
        self.push_call(user_function.function_index);
        let return_value_local = self.allocate_temp_local();
        self.push_local_set(return_value_local);
        self.sync_user_function_capture_source_bindings(
            &prepared_capture_bindings,
            &assigned_nonlocal_bindings,
            &call_effect_nonlocal_bindings,
            &updated_nonlocal_bindings,
            updated_bindings.as_ref(),
        )?;
        self.restore_user_function_capture_bindings(&prepared_capture_bindings);
        additional_call_effect_nonlocal_bindings = self
            .sync_snapshot_user_function_call_effect_bindings(
                &additional_call_effect_nonlocal_bindings,
                updated_bindings.as_ref(),
                assigned_nonlocal_binding_results.as_ref(),
            )?;
        if !additional_call_effect_nonlocal_bindings.is_empty() {
            let preserved_kinds = additional_call_effect_nonlocal_bindings
                .iter()
                .filter_map(|name| {
                    self.lookup_identifier_kind(name)
                        .map(|kind| (name.clone(), kind))
                })
                .collect::<HashMap<_, _>>();
            self.invalidate_static_binding_metadata_for_names_with_preserved_kinds(
                &additional_call_effect_nonlocal_bindings,
                &preserved_kinds,
            );
        }
        self.sync_argument_iterator_bindings_for_user_call(
            user_function,
            &bound_argument_expressions,
        );
        if let Some(saved_new_target_local) = saved_new_target_local {
            self.push_local_get(saved_new_target_local);
            self.push_global_set(CURRENT_NEW_TARGET_GLOBAL_INDEX);
        }
        if let Some(saved_this_local) = saved_this_local {
            self.push_local_get(saved_this_local);
            self.push_global_set(CURRENT_THIS_GLOBAL_INDEX);
        }
        if user_function.is_async() {
            self.push_global_get(THROW_TAG_GLOBAL_INDEX);
            self.push_i32_const(0);
            self.push_binary_op(BinaryOp::NotEqual)?;
            self.state.emission.output.instructions.push(0x04);
            self.state
                .emission
                .output
                .instructions
                .push(EMPTY_BLOCK_TYPE);
            self.push_control_frame();
            self.clear_global_throw_state();
            self.state.emission.output.instructions.push(0x0b);
            self.pop_control_frame();
            self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
            return Ok(());
        }
        self.emit_check_global_throw_for_user_call()?;
        self.push_local_get(return_value_local);
        Ok(())
    }
}
