use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_user_function_call_with_new_target(
        &mut self,
        user_function: &UserFunction,
        arguments: &[CallArgument],
        new_target_value: i32,
    ) -> DirectResult<()> {
        self.emit_user_function_call_with_new_target_and_this(
            user_function,
            arguments,
            new_target_value,
            JS_TYPEOF_OBJECT_TAG,
        )
    }

    pub(in crate::backend::direct_wasm) fn emit_user_function_call_with_new_target_and_this(
        &mut self,
        user_function: &UserFunction,
        arguments: &[CallArgument],
        new_target_value: i32,
        this_value: i32,
    ) -> DirectResult<()> {
        let expanded_arguments = self.expand_call_arguments(arguments);
        let materialized_inline_arguments = expanded_arguments
            .iter()
            .map(|argument| self.materialize_static_expression(argument))
            .collect::<Vec<_>>();
        let inline_this_expression = if this_value == JS_UNDEFINED_TAG {
            Expression::Undefined
        } else {
            Expression::This
        };
        let static_this_expression =
            self.resolve_static_snapshot_this_expression(&inline_this_expression);
        if self.emit_deferred_generator_call_result(user_function, &expanded_arguments)? {
            return Ok(());
        }
        if new_target_value == JS_UNDEFINED_TAG
            && !user_function.lexical_this
            && self.can_inline_user_function_call_with_explicit_call_frame(
                user_function,
                &materialized_inline_arguments,
                &static_this_expression,
            )
        {
            let result_local = self.allocate_temp_local();
            if self.emit_inline_user_function_summary_with_explicit_call_frame(
                user_function,
                &expanded_arguments,
                &static_this_expression,
                result_local,
            )? {
                self.push_local_get(result_local);
                return Ok(());
            }
        }
        if new_target_value == JS_UNDEFINED_TAG
            && self.can_inline_user_function_call(user_function, &expanded_arguments)
        {
            for argument in &expanded_arguments {
                self.emit_numeric_expression(argument)?;
                self.state.emission.output.instructions.push(0x1a);
            }
            if self.emit_inline_user_function_summary_with_arguments(
                user_function,
                &expanded_arguments,
            )? {
                return Ok(());
            }
        }

        let prepared_capture_bindings =
            self.prepare_user_function_capture_bindings(user_function)?;

        self.emit_prepared_user_function_call_with_new_target_and_this(
            user_function,
            &expanded_arguments,
            new_target_value,
            this_value,
            prepared_capture_bindings,
        )
    }

    #[allow(dead_code)]
    pub(in crate::backend::direct_wasm) fn emit_user_function_call_without_inline_with_new_target_and_this(
        &mut self,
        user_function: &UserFunction,
        arguments: &[CallArgument],
        new_target_value: i32,
        this_value: i32,
    ) -> DirectResult<()> {
        let expanded_arguments = self.expand_call_arguments(arguments);
        let prepared_capture_bindings =
            self.prepare_user_function_capture_bindings(user_function)?;
        self.emit_prepared_user_function_call_with_new_target_and_this(
            user_function,
            &expanded_arguments,
            new_target_value,
            this_value,
            prepared_capture_bindings,
        )
    }
}
