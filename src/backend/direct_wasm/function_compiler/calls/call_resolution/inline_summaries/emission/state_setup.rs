use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn prepare_inline_summary_emission_state(
        &mut self,
        user_function: &UserFunction,
        arguments: &[Expression],
        this_binding: &Expression,
    ) -> DirectResult<InlineSummaryEmissionState> {
        let prepared_capture_bindings =
            self.prepare_user_function_capture_bindings(user_function)?;
        let synced_capture_source_bindings =
            self.synced_prepared_user_function_capture_source_bindings(&prepared_capture_bindings);
        let capture_snapshot =
            self.snapshot_user_function_capture_source_bindings(&prepared_capture_bindings);
        let updated_bindings = self
            .resolve_bound_snapshot_user_function_result_with_arguments_and_this(
                &user_function.name,
                &capture_snapshot,
                arguments,
                this_binding,
            )
            .map(|(_, updated_bindings)| updated_bindings);
        let assigned_nonlocal_bindings =
            self.collect_user_function_assigned_nonlocal_bindings(user_function);
        let mut call_effect_nonlocal_bindings =
            self.collect_user_function_call_effect_nonlocal_bindings(user_function);
        call_effect_nonlocal_bindings.extend(
            self.collect_user_function_argument_call_effect_nonlocal_bindings(
                user_function,
                arguments,
            ),
        );
        let assigned_nonlocal_binding_results = self
            .assigned_nonlocal_binding_results(&user_function.name)
            .cloned();
        let additional_call_effect_nonlocal_bindings = call_effect_nonlocal_bindings
            .iter()
            .filter(|name| !synced_capture_source_bindings.contains(*name))
            .cloned()
            .collect::<HashSet<_>>();
        let updated_nonlocal_bindings =
            self.collect_user_function_updated_nonlocal_bindings(user_function);
        self.emit_prepare_user_function_capture_globals(&user_function.name)?;

        let arguments_binding = Expression::Array(
            arguments
                .iter()
                .cloned()
                .map(crate::ir::hir::ArrayElement::Expression)
                .collect(),
        );
        let (call_arguments, inline_parameter_scope_names) = self
            .prepare_inline_summary_call_arguments(user_function, arguments, &arguments_binding)?;

        Ok(InlineSummaryEmissionState {
            prepared_capture_bindings,
            assigned_nonlocal_bindings,
            call_effect_nonlocal_bindings,
            assigned_nonlocal_binding_results,
            additional_call_effect_nonlocal_bindings,
            updated_nonlocal_bindings,
            updated_bindings,
            arguments_binding,
            call_arguments,
            inline_parameter_scope_names,
        })
    }

    fn prepare_inline_summary_call_arguments(
        &mut self,
        user_function: &UserFunction,
        arguments: &[Expression],
        arguments_binding: &Expression,
    ) -> DirectResult<(Vec<CallArgument>, Vec<String>)> {
        let mut call_arguments = Vec::new();
        let mut inline_parameter_scope_names = Vec::new();
        let visible_param_count = user_function.visible_param_count() as usize;
        for (param_index, param_name) in user_function
            .params
            .iter()
            .take(visible_param_count)
            .enumerate()
        {
            let argument = arguments
                .get(param_index)
                .cloned()
                .unwrap_or(Expression::Undefined);
            let hidden_name = self.allocate_named_hidden_local(
                &format!("inline_param_{param_name}"),
                self.infer_value_kind(&argument)
                    .unwrap_or(StaticValueKind::Unknown),
            );
            let hidden_local = self
                .state
                .runtime
                .locals
                .get(&hidden_name)
                .copied()
                .expect("inline parameter local must exist");
            self.emit_numeric_expression(&argument)?;
            self.push_local_set(hidden_local);
            self.update_capture_slot_binding_from_expression(&hidden_name, &argument)?;
            self.state
                .emission
                .lexical_scopes
                .active_scoped_lexical_bindings
                .entry(param_name.clone())
                .or_default()
                .push(hidden_name.clone());
            call_arguments.push(CallArgument::Expression(Expression::Identifier(
                hidden_name,
            )));
            inline_parameter_scope_names.push(param_name.clone());
        }
        let arguments_shadowed = user_function.body_declares_arguments_binding
            || user_function
                .params
                .iter()
                .any(|param| param == "arguments");
        if !arguments_shadowed {
            let hidden_name =
                self.allocate_named_hidden_local("inline_arguments", StaticValueKind::Object);
            let hidden_local = self
                .state
                .runtime
                .locals
                .get(&hidden_name)
                .copied()
                .expect("inline arguments local must exist");
            self.emit_numeric_expression(arguments_binding)?;
            self.push_local_set(hidden_local);
            self.update_capture_slot_binding_from_expression(&hidden_name, arguments_binding)?;
            self.state
                .emission
                .lexical_scopes
                .active_scoped_lexical_bindings
                .entry("arguments".to_string())
                .or_default()
                .push(hidden_name);
            inline_parameter_scope_names.push("arguments".to_string());
        }
        Ok((call_arguments, inline_parameter_scope_names))
    }

    pub(super) fn abort_inline_summary_emission_state(
        &mut self,
        state: &InlineSummaryEmissionState,
    ) {
        self.pop_scoped_lexical_bindings(&state.inline_parameter_scope_names);
        self.restore_user_function_capture_bindings(&state.prepared_capture_bindings);
    }

    pub(super) fn finalize_inline_summary_emission_state(
        &mut self,
        user_function: &UserFunction,
        arguments: &[Expression],
        state: &mut InlineSummaryEmissionState,
    ) -> DirectResult<()> {
        self.pop_scoped_lexical_bindings(&state.inline_parameter_scope_names);
        self.sync_user_function_capture_source_bindings(
            &state.prepared_capture_bindings,
            &state.assigned_nonlocal_bindings,
            &state.call_effect_nonlocal_bindings,
            &state.updated_nonlocal_bindings,
            state.updated_bindings.as_ref(),
        )?;
        self.restore_user_function_capture_bindings(&state.prepared_capture_bindings);
        state.additional_call_effect_nonlocal_bindings = self
            .sync_snapshot_user_function_call_effect_bindings(
                &state.additional_call_effect_nonlocal_bindings,
                state.updated_bindings.as_ref(),
                state.assigned_nonlocal_binding_results.as_ref(),
            )?;
        if !state.additional_call_effect_nonlocal_bindings.is_empty() {
            let preserved_kinds = state
                .additional_call_effect_nonlocal_bindings
                .iter()
                .filter_map(|name| {
                    self.lookup_identifier_kind(name)
                        .map(|kind| (name.clone(), kind))
                })
                .collect::<HashMap<_, _>>();
            self.invalidate_static_binding_metadata_for_names_with_preserved_kinds(
                &state.additional_call_effect_nonlocal_bindings,
                &preserved_kinds,
            );
        }
        self.sync_argument_iterator_bindings_for_user_call(user_function, arguments);
        Ok(())
    }
}
