use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_user_function_call_with_new_target_and_this_expression_and_bound_captures_from_argument_locals(
        &mut self,
        user_function: &UserFunction,
        argument_locals: &[u32],
        argument_count: usize,
        new_target_value: i32,
        this_expression: &Expression,
        capture_slots: &BTreeMap<String, String>,
    ) -> DirectResult<()> {
        let runtime_only_parameter_iterator_call = user_function.has_lowered_pattern_parameters()
            || !self
                .user_function_parameter_iterator_consumption_indices(user_function)
                .is_empty();
        let (
            prepared_capture_bindings,
            synced_capture_source_bindings,
            saved_new_target_local,
            saved_this_local,
        ) = self.prepare_bound_user_function_call_context(
            user_function,
            capture_slots,
            new_target_value,
            this_expression,
        )?;

        let capture_snapshot = capture_slots
            .iter()
            .map(|(capture_name, slot_name)| {
                (
                    capture_name.clone(),
                    self.snapshot_bound_capture_slot_expression(slot_name),
                )
            })
            .collect::<HashMap<_, _>>();
        let bound_argument_expressions = argument_locals
            .iter()
            .take(argument_count)
            .map(|argument_local| {
                self.state
                    .runtime
                    .locals
                    .iter()
                    .find_map(|(name, local)| {
                        (*local == *argument_local).then_some(Expression::Identifier(name.clone()))
                    })
                    .unwrap_or(Expression::Undefined)
            })
            .collect::<Vec<_>>();
        let static_result = if runtime_only_parameter_iterator_call {
            None
        } else {
            self.resolve_bound_snapshot_user_function_result_with_arguments_and_this(
                &user_function.name,
                &capture_snapshot,
                &bound_argument_expressions,
                this_expression,
            )
        };
        self.state
            .speculation
            .static_semantics
            .last_bound_user_function_call =
            (!runtime_only_parameter_iterator_call).then(|| BoundUserFunctionCallSnapshot {
                function_name: user_function.name.clone(),
                source_expression: None,
                result_expression: static_result.as_ref().map(|(result, _)| result.clone()),
                updated_bindings: static_result
                    .as_ref()
                    .map(|(_, updated_bindings)| updated_bindings.clone())
                    .unwrap_or_else(|| capture_snapshot.clone()),
            });
        let mut call_effect_nonlocal_bindings = if runtime_only_parameter_iterator_call {
            HashSet::new()
        } else {
            self.collect_user_function_call_effect_nonlocal_bindings(user_function)
        };
        if !runtime_only_parameter_iterator_call {
            call_effect_nonlocal_bindings.extend(
                self.collect_user_function_argument_call_effect_nonlocal_bindings(
                    user_function,
                    &bound_argument_expressions,
                ),
            );
        }
        let assigned_nonlocal_binding_results = if runtime_only_parameter_iterator_call {
            None
        } else {
            self.assigned_nonlocal_binding_results(&user_function.name)
                .cloned()
        };
        let additional_call_effect_nonlocal_bindings = if runtime_only_parameter_iterator_call {
            HashSet::new()
        } else {
            let mut names = call_effect_nonlocal_bindings
                .iter()
                .filter(|name| !synced_capture_source_bindings.contains(*name))
                .cloned()
                .collect::<HashSet<_>>();
            names.extend(
                self.collect_snapshot_updated_nonlocal_bindings(
                    user_function,
                    static_result
                        .as_ref()
                        .map(|(_, updated_bindings)| updated_bindings),
                ),
            );
            names
        };

        self.emit_prepare_bound_user_function_capture_globals(&prepared_capture_bindings)?;

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
        let updated_bindings = self
            .state
            .speculation
            .static_semantics
            .last_bound_user_function_call
            .as_ref()
            .and_then(|snapshot| {
                (snapshot.function_name == user_function.name)
                    .then_some(snapshot.updated_bindings.clone())
            });

        self.finalize_bound_user_function_call(
            user_function,
            &prepared_capture_bindings,
            updated_bindings,
            additional_call_effect_nonlocal_bindings,
            assigned_nonlocal_binding_results,
            saved_new_target_local,
            saved_this_local,
            return_value_local,
            &bound_argument_expressions,
        )
    }
}
