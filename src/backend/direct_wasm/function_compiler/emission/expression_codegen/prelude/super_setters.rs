use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_user_super_setter_call(
        &self,
        base: &Expression,
        property: &Expression,
    ) -> Option<(UserFunction, Option<BTreeMap<String, String>>)> {
        let LocalFunctionBinding::User(function_name) =
            self.resolve_member_setter_binding(base, property)?
        else {
            return None;
        };
        let user_function = self.user_function(&function_name)?.clone();
        let capture_slots = self.resolve_member_function_capture_slots(base, property);
        Some((user_function, capture_slots))
    }

    pub(in crate::backend::direct_wasm) fn emit_super_member_user_setter_call(
        &mut self,
        user_function: &UserFunction,
        capture_slots: Option<&BTreeMap<String, String>>,
        value_local: u32,
    ) -> DirectResult<()> {
        if capture_slots.is_none()
            && self.with_suspended_with_scopes(|compiler| {
                compiler.emit_inline_user_function_summary_with_argument_locals(
                    user_function,
                    &[value_local],
                    1,
                )
            })?
        {
            self.state.emission.output.instructions.push(0x1a);
            return Ok(());
        }
        if let Some(capture_slots) = capture_slots {
            self.emit_user_function_call_with_new_target_and_this_expression_and_bound_captures_from_argument_locals(
                user_function,
                &[value_local],
                1,
                JS_UNDEFINED_TAG,
                &Expression::This,
                capture_slots,
            )?;
        } else {
            self.emit_user_function_call_with_new_target_and_this_expression_from_argument_locals(
                user_function,
                &[value_local],
                1,
                JS_UNDEFINED_TAG,
                &Expression::This,
            )?;
        }
        self.state.emission.output.instructions.push(0x1a);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn resolve_user_super_setter_variants(
        &self,
        binding: &GlobalObjectRuntimePrototypeBinding,
        property: &Expression,
    ) -> Option<Vec<(UserFunction, Option<BTreeMap<String, String>>)>> {
        let mut variants = Vec::with_capacity(binding.variants.len());
        for prototype in &binding.variants {
            let prototype = prototype.as_ref()?;
            let Some(variant) = self.resolve_user_super_setter_call(prototype, property) else {
                return None;
            };
            variants.push(variant);
        }
        Some(variants)
    }

    pub(in crate::backend::direct_wasm) fn emit_super_member_user_setter_call_via_runtime_prototype_state(
        &mut self,
        variants: &[(UserFunction, Option<BTreeMap<String, String>>)],
        state_local: u32,
        value_local: u32,
    ) -> DirectResult<()> {
        let mut open_frames = 0;
        for (variant_index, (user_function, capture_slots)) in variants.iter().enumerate() {
            self.push_local_get(state_local);
            self.push_i32_const(variant_index as i32);
            self.push_binary_op(BinaryOp::Equal)?;
            self.state.emission.output.instructions.push(0x04);
            self.state
                .emission
                .output
                .instructions
                .push(EMPTY_BLOCK_TYPE);
            self.push_control_frame();
            open_frames += 1;
            self.emit_super_member_user_setter_call(
                user_function,
                capture_slots.as_ref(),
                value_local,
            )?;
            self.state.emission.output.instructions.push(0x05);
        }

        for _ in 0..open_frames {
            self.state.emission.output.instructions.push(0x0b);
            self.pop_control_frame();
        }

        for (user_function, _) in variants {
            self.invalidate_user_function_assigned_nonlocal_bindings(user_function);
        }

        Ok(())
    }
}
