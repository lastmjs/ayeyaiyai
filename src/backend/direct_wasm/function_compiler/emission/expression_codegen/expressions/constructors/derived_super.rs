use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_derived_constructor_super_call(
        &mut self,
        user_function: &UserFunction,
        arguments: &[CallArgument],
    ) -> DirectResult<()> {
        let expanded_arguments = self.expand_call_arguments(arguments);
        let runtime_arguments = expanded_arguments
            .iter()
            .enumerate()
            .map(|(index, argument)| {
                let argument_local = self.allocate_temp_local();
                self.emit_numeric_expression(argument)?;
                self.push_local_set(argument_local);
                let hidden_name = self.allocate_named_hidden_local(
                    &format!("derived_super_arg_{index}"),
                    self.infer_value_kind(argument)
                        .unwrap_or(StaticValueKind::Unknown),
                );
                let hidden_local = self
                    .state
                    .runtime
                    .locals
                    .get(&hidden_name)
                    .copied()
                    .expect("fresh derived super argument local must exist");
                self.push_local_get(argument_local);
                self.push_local_set(hidden_local);
                Ok(CallArgument::Expression(Expression::Identifier(
                    hidden_name,
                )))
            })
            .collect::<DirectResult<Vec<_>>>()?;

        self.emit_user_function_call_with_current_new_target_and_this_expression(
            user_function,
            &runtime_arguments,
            &Expression::Object(Vec::new()),
        )?;
        self.finish_derived_super_call_return(&runtime_arguments, |compiler| {
            compiler.sync_derived_constructor_this_binding_after_super_call(
                user_function,
                &runtime_arguments,
            );
        })
    }

    pub(in crate::backend::direct_wasm) fn emit_derived_constructor_builtin_super_call(
        &mut self,
        function_name: &str,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        if !self.emit_builtin_call(function_name, arguments)? {
            return Ok(false);
        }
        self.finish_derived_super_call_return(arguments, |compiler| {
            compiler.sync_derived_constructor_this_binding_after_builtin_super_call();
        })?;
        Ok(true)
    }

    fn finish_derived_super_call_return(
        &mut self,
        _arguments: &[CallArgument],
        sync_this: impl FnOnce(&mut Self),
    ) -> DirectResult<()> {
        let return_value_local = self.allocate_temp_local();
        self.push_local_set(return_value_local);
        self.push_local_get(return_value_local);
        let return_value_visible_local = self.allocate_temp_local();
        self.push_local_set(return_value_visible_local);

        self.push_global_get(CURRENT_THIS_GLOBAL_INDEX);
        self.push_i32_const(JS_UNDEFINED_TAG);
        self.push_binary_op(BinaryOp::NotEqual)?;
        self.state.emission.output.instructions.push(0x04);
        self.state.emission.output.instructions.push(I32_TYPE);
        self.push_control_frame();
        self.emit_named_error_throw("ReferenceError")?;
        self.push_i32_const(JS_UNDEFINED_TAG);
        self.state.emission.output.instructions.push(0x05);

        sync_this(self);

        let initialized_this_local = self.allocate_temp_local();
        self.push_local_get(return_value_visible_local);
        self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
        self.push_binary_op(BinaryOp::Equal)?;
        self.state.emission.output.instructions.push(0x04);
        self.state.emission.output.instructions.push(I32_TYPE);
        self.push_control_frame();
        self.push_local_get(return_value_visible_local);
        self.state.emission.output.instructions.push(0x05);
        self.push_local_get(return_value_visible_local);
        self.push_i32_const(JS_TYPEOF_FUNCTION_TAG);
        self.push_binary_op(BinaryOp::Equal)?;
        self.state.emission.output.instructions.push(0x04);
        self.state.emission.output.instructions.push(I32_TYPE);
        self.push_control_frame();
        self.push_local_get(return_value_visible_local);
        self.state.emission.output.instructions.push(0x05);
        self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
        self.state.emission.output.instructions.push(0x0b);
        self.pop_control_frame();
        self.state.emission.output.instructions.push(0x0b);
        self.pop_control_frame();
        self.push_local_set(initialized_this_local);
        self.push_local_get(initialized_this_local);
        self.push_global_set(CURRENT_THIS_GLOBAL_INDEX);
        self.push_local_get(initialized_this_local);
        self.state.emission.output.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(())
    }
}
