use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_user_function_call_with_current_new_target_and_this_expression(
        &mut self,
        user_function: &UserFunction,
        arguments: &[CallArgument],
        this_expression: &Expression,
    ) -> DirectResult<()> {
        let expanded_arguments = self.expand_call_arguments(arguments);
        let prepared_capture_bindings =
            self.prepare_user_function_capture_bindings(user_function)?;
        let assigned_nonlocal_bindings =
            self.collect_user_function_assigned_nonlocal_bindings(user_function);
        let mut call_effect_nonlocal_bindings =
            self.collect_user_function_call_effect_nonlocal_bindings(user_function);
        call_effect_nonlocal_bindings.extend(
            self.collect_user_function_argument_call_effect_nonlocal_bindings(
                user_function,
                &expanded_arguments,
            ),
        );
        let updated_nonlocal_bindings =
            self.collect_user_function_updated_nonlocal_bindings(user_function);

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
        let tracked_extra_indices = user_function
            .extra_argument_indices
            .iter()
            .map(|index| *index as usize)
            .collect::<HashSet<_>>();
        let mut argument_locals = HashMap::new();

        for (argument_index, argument) in expanded_arguments.iter().enumerate() {
            if argument_index < visible_param_count
                || tracked_extra_indices.contains(&argument_index)
            {
                let argument_local = self.allocate_temp_local();
                self.emit_numeric_expression(argument)?;
                self.push_local_set(argument_local);
                argument_locals.insert(argument_index, argument_local);
            } else {
                self.emit_numeric_expression(argument)?;
                self.state.emission.output.instructions.push(0x1a);
            }
        }

        for argument_index in 0..visible_param_count {
            if let Some(argument_local) = argument_locals.get(&argument_index).copied() {
                self.push_local_get(argument_local);
            } else {
                self.push_i32_const(JS_UNDEFINED_TAG);
            }
        }

        self.push_i32_const(expanded_arguments.len() as i32);

        for index in &user_function.extra_argument_indices {
            if let Some(argument_local) = argument_locals.get(&(*index as usize)).copied() {
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
            None,
        )?;
        self.restore_user_function_capture_bindings(&prepared_capture_bindings);
        self.sync_argument_iterator_bindings_for_user_call(user_function, &expanded_arguments);

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
