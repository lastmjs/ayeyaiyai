use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn prepare_bound_user_function_call_context(
        &mut self,
        user_function: &UserFunction,
        capture_slots: &BTreeMap<String, String>,
        new_target_value: i32,
        this_expression: &Expression,
    ) -> DirectResult<(
        Vec<PreparedBoundCaptureBinding>,
        HashSet<String>,
        Option<u32>,
        Option<u32>,
    )> {
        let prepared_capture_bindings =
            self.prepare_bound_user_function_capture_bindings(user_function, capture_slots)?;
        let synced_capture_source_bindings = self
            .synced_prepared_bound_user_function_capture_source_bindings(
                &prepared_capture_bindings,
            );

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

        Ok((
            prepared_capture_bindings,
            synced_capture_source_bindings,
            saved_new_target_local,
            saved_this_local,
        ))
    }

    pub(in crate::backend::direct_wasm) fn finalize_bound_user_function_call(
        &mut self,
        user_function: &UserFunction,
        prepared_capture_bindings: &[PreparedBoundCaptureBinding],
        updated_bindings: Option<HashMap<String, Expression>>,
        additional_call_effect_nonlocal_bindings: HashSet<String>,
        assigned_nonlocal_binding_results: Option<HashMap<String, Expression>>,
        saved_new_target_local: Option<u32>,
        saved_this_local: Option<u32>,
        return_value_local: u32,
        argument_expressions: &[Expression],
    ) -> DirectResult<()> {
        self.sync_bound_user_function_capture_slots(
            prepared_capture_bindings,
            updated_bindings.as_ref(),
        )?;
        self.restore_bound_user_function_capture_bindings(prepared_capture_bindings);

        let additional_call_effect_nonlocal_bindings = self
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

        self.sync_argument_iterator_bindings_for_user_call(user_function, argument_expressions);

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
