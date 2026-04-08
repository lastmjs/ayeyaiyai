use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_prepared_user_function_call_with_new_target_and_this(
        &mut self,
        user_function: &UserFunction,
        expanded_arguments: &[Expression],
        new_target_value: i32,
        this_value: i32,
        prepared_capture_bindings: Vec<PreparedCaptureBinding>,
    ) -> DirectResult<()> {
        let runtime_only_parameter_iterator_call = user_function.has_lowered_pattern_parameters()
            || !self
                .user_function_parameter_iterator_consumption_indices(user_function)
                .is_empty();
        let synced_capture_source_bindings =
            self.synced_prepared_user_function_capture_source_bindings(&prepared_capture_bindings);
        let capture_snapshot =
            self.snapshot_user_function_capture_source_bindings(&prepared_capture_bindings);
        let this_expression = if this_value == JS_UNDEFINED_TAG {
            Expression::Undefined
        } else {
            Expression::This
        };
        let static_this_expression = self.resolve_static_snapshot_this_expression(&this_expression);
        let static_result =
            if !runtime_only_parameter_iterator_call && new_target_value == JS_UNDEFINED_TAG {
                self.resolve_bound_snapshot_user_function_result_with_arguments_and_this(
                    &user_function.name,
                    &capture_snapshot,
                    expanded_arguments,
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
            .last_bound_user_function_call =
            (!runtime_only_parameter_iterator_call).then(|| BoundUserFunctionCallSnapshot {
                function_name: user_function.name.clone(),
                source_expression: None,
                result_expression: static_result.as_ref().map(|(result, _)| result.clone()),
                updated_bindings: updated_bindings
                    .clone()
                    .unwrap_or_else(|| capture_snapshot.clone()),
            });
        let assigned_nonlocal_bindings = if runtime_only_parameter_iterator_call {
            HashSet::new()
        } else {
            self.collect_user_function_assigned_nonlocal_bindings(user_function)
        };
        let mut call_effect_nonlocal_bindings = if runtime_only_parameter_iterator_call {
            HashSet::new()
        } else {
            self.collect_user_function_call_effect_nonlocal_bindings(user_function)
        };
        if !runtime_only_parameter_iterator_call {
            call_effect_nonlocal_bindings.extend(
                self.collect_user_function_argument_call_effect_nonlocal_bindings(
                    user_function,
                    expanded_arguments,
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
            names.extend(self.collect_snapshot_updated_nonlocal_bindings(
                user_function,
                updated_bindings.as_ref(),
            ));
            names
        };
        let updated_nonlocal_bindings = if runtime_only_parameter_iterator_call {
            HashSet::new()
        } else {
            self.collect_user_function_updated_nonlocal_bindings(user_function)
        };
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
            self.push_global_get(CURRENT_THIS_GLOBAL_INDEX);
            self.push_local_set(saved_local);
            self.push_i32_const(this_value);
            self.push_global_set(CURRENT_THIS_GLOBAL_INDEX);
            Some(saved_local)
        };

        self.emit_prepare_user_function_capture_globals(&user_function.name)?;
        let return_value_local = self.emit_user_function_runtime_call_from_expanded_arguments(
            user_function,
            expanded_arguments,
        )?;
        self.finalize_user_function_call(
            user_function,
            &prepared_capture_bindings,
            &assigned_nonlocal_bindings,
            &call_effect_nonlocal_bindings,
            &updated_nonlocal_bindings,
            updated_bindings.as_ref(),
            additional_call_effect_nonlocal_bindings,
            assigned_nonlocal_binding_results,
            saved_new_target_local,
            saved_this_local,
            return_value_local,
            expanded_arguments,
        )
    }
}
