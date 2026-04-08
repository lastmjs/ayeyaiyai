use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_deferred_generator_call_result(
        &mut self,
        user_function: &UserFunction,
        expanded_arguments: &[Expression],
    ) -> DirectResult<bool> {
        let generator_call = Expression::Call {
            callee: Box::new(Expression::Identifier(user_function.name.clone())),
            arguments: expanded_arguments
                .iter()
                .cloned()
                .map(CallArgument::Expression)
                .collect(),
        };
        if (user_function.is_generator()
            && self
                .resolve_simple_generator_source(&generator_call)
                .is_some())
            || (matches!(user_function.kind, FunctionKind::AsyncGenerator)
                && self
                    .resolve_async_yield_delegate_generator_plan(
                        &generator_call,
                        "__ayy_async_delegate_completion",
                    )
                    .is_some())
        {
            self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
            return Ok(true);
        }
        Ok(false)
    }

    pub(in crate::backend::direct_wasm) fn emit_user_function_call(
        &mut self,
        user_function: &UserFunction,
        arguments: &[CallArgument],
    ) -> DirectResult<()> {
        self.emit_user_function_call_with_new_target_and_this(
            user_function,
            arguments,
            JS_UNDEFINED_TAG,
            if user_function.strict {
                JS_UNDEFINED_TAG
            } else {
                JS_TYPEOF_OBJECT_TAG
            },
        )
    }

    pub(in crate::backend::direct_wasm) fn emit_dynamic_user_function_call(
        &mut self,
        callee: &Expression,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        let callee_local = self.allocate_temp_local();
        self.emit_numeric_expression(callee)?;
        self.push_local_set(callee_local);

        self.push_local_get(callee_local);
        self.push_i32_const(JS_BUILTIN_EVAL_VALUE);
        self.push_binary_op(BinaryOp::Equal)?;
        self.state.emission.output.instructions.push(0x04);
        self.state.emission.output.instructions.push(I32_TYPE);
        self.push_control_frame();
        self.emit_indirect_eval_call(arguments)?;
        self.state.emission.output.instructions.push(0x05);

        if self
            .backend
            .function_registry
            .catalog
            .user_functions
            .is_empty()
        {
            self.push_i32_const(JS_UNDEFINED_TAG);
            self.state.emission.output.instructions.push(0x0b);
            self.pop_control_frame();
            return Ok(true);
        }

        let expanded_arguments = self.expand_call_arguments(arguments);
        let mut call_arguments = Vec::with_capacity(expanded_arguments.len());
        for (index, argument) in expanded_arguments.iter().enumerate() {
            let argument_value_local = self.allocate_temp_local();
            self.emit_numeric_expression(argument)?;
            self.push_local_set(argument_value_local);

            let hidden_name = self.allocate_named_hidden_local(
                &format!("dynamic_call_arg_{index}"),
                self.infer_value_kind(argument)
                    .unwrap_or(StaticValueKind::Unknown),
            );
            let hidden_local = self
                .state
                .runtime
                .locals
                .get(&hidden_name)
                .copied()
                .expect("fresh dynamic call hidden local must exist");
            self.push_local_get(argument_value_local);
            self.push_local_set(hidden_local);
            call_arguments.push(CallArgument::Expression(Expression::Identifier(
                hidden_name,
            )));
        }

        let user_functions = self.user_functions();
        for (index, user_function) in user_functions.iter().enumerate() {
            self.push_local_get(callee_local);
            self.push_i32_const(user_function_runtime_value(user_function));
            self.push_binary_op(BinaryOp::Equal)?;
            self.state.emission.output.instructions.push(0x04);
            self.state.emission.output.instructions.push(I32_TYPE);
            self.push_control_frame();
            self.emit_user_function_call_without_inline_with_new_target_and_this(
                user_function,
                &call_arguments,
                JS_UNDEFINED_TAG,
                if user_function.strict {
                    JS_UNDEFINED_TAG
                } else {
                    JS_TYPEOF_OBJECT_TAG
                },
            )?;
            self.state.emission.output.instructions.push(0x05);
            if index + 1 == user_functions.len() {
                self.push_i32_const(JS_UNDEFINED_TAG);
            }
        }
        for _ in 0..user_functions.len() {
            self.state.emission.output.instructions.push(0x0b);
            self.pop_control_frame();
        }
        self.state.emission.output.instructions.push(0x0b);
        self.pop_control_frame();

        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_dynamic_super_call(
        &mut self,
        callee: &Expression,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        let callee_local = self.allocate_temp_local();
        self.emit_numeric_expression(callee)?;
        self.push_local_set(callee_local);

        if self
            .backend
            .function_registry
            .catalog
            .user_functions
            .is_empty()
        {
            return Ok(false);
        }

        let expanded_arguments = self.expand_call_arguments(arguments);
        let mut call_arguments = Vec::with_capacity(expanded_arguments.len());
        for (index, argument) in expanded_arguments.iter().enumerate() {
            let argument_value_local = self.allocate_temp_local();
            self.emit_numeric_expression(argument)?;
            self.push_local_set(argument_value_local);

            let hidden_name = self.allocate_named_hidden_local(
                &format!("dynamic_super_arg_{index}"),
                self.infer_value_kind(argument)
                    .unwrap_or(StaticValueKind::Unknown),
            );
            let hidden_local = self
                .state
                .runtime
                .locals
                .get(&hidden_name)
                .copied()
                .expect("fresh dynamic super hidden local must exist");
            self.push_local_get(argument_value_local);
            self.push_local_set(hidden_local);
            call_arguments.push(CallArgument::Expression(Expression::Identifier(
                hidden_name,
            )));
        }

        let constructible_user_functions = self
            .backend
            .function_registry
            .catalog
            .user_functions
            .iter()
            .filter(|user_function| user_function.is_constructible())
            .cloned()
            .collect::<Vec<_>>();
        if constructible_user_functions.is_empty() {
            return Ok(false);
        }

        for (index, user_function) in constructible_user_functions.iter().enumerate() {
            self.push_local_get(callee_local);
            self.push_i32_const(user_function_runtime_value(user_function));
            self.push_binary_op(BinaryOp::Equal)?;
            self.state.emission.output.instructions.push(0x04);
            self.state.emission.output.instructions.push(I32_TYPE);
            self.push_control_frame();
            if self.current_function_is_derived_constructor() {
                self.emit_derived_constructor_super_call(user_function, &call_arguments)?;
            } else {
                self.emit_user_function_call_with_current_new_target_and_this_expression(
                    user_function,
                    &call_arguments,
                    &Expression::This,
                )?;
            }
            self.state.emission.output.instructions.push(0x05);
            if index + 1 == constructible_user_functions.len() {
                self.push_i32_const(JS_UNDEFINED_TAG);
            }
        }
        for _ in 0..constructible_user_functions.len() {
            self.state.emission.output.instructions.push(0x0b);
            self.pop_control_frame();
        }

        Ok(true)
    }
}
