use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_assign_super_member_expression(
        &mut self,
        property: &Expression,
        value: &Expression,
    ) -> DirectResult<()> {
        let runtime_prototype_binding =
            self.resolve_super_runtime_prototype_binding_with_context(self.current_function_name());
        let runtime_state_local = runtime_prototype_binding
            .as_ref()
            .and_then(|(_, binding)| binding.global_index)
            .map(|global_index| {
                let local = self.allocate_temp_local();
                self.push_global_get(global_index);
                self.push_local_set(local);
                local
            });

        let resolved_property = self.emit_property_key_expression_effects(property)?;
        let Some(effective_property) = resolved_property.as_ref() else {
            self.emit_numeric_expression(value)?;
            self.state.emission.output.instructions.push(0x1a);
            self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
            return Ok(());
        };
        let super_base =
            self.resolve_super_base_expression_with_context(self.current_function_name());

        if let Some((_, binding)) = runtime_prototype_binding.as_ref()
            && let Some(state_local) = runtime_state_local
            && let Some(variants) =
                self.resolve_user_super_setter_variants(binding, effective_property)
        {
            let value_local = self.allocate_temp_local();
            self.emit_numeric_expression(value)?;
            self.push_local_set(value_local);
            self.emit_super_member_user_setter_call_via_runtime_prototype_state(
                &variants,
                state_local,
                value_local,
            )?;
            self.push_local_get(value_local);
            return Ok(());
        }

        if runtime_prototype_binding.is_none()
            && let Some(super_base) = super_base.as_ref()
            && let Some((user_function, capture_slots)) =
                self.resolve_user_super_setter_call(super_base, effective_property)
        {
            let value_local = self.allocate_temp_local();
            self.emit_numeric_expression(value)?;
            self.push_local_set(value_local);
            self.emit_super_member_user_setter_call(
                &user_function,
                capture_slots.as_ref(),
                value_local,
            )?;
            self.push_local_get(value_local);
            return Ok(());
        }

        self.emit_numeric_expression(&Expression::AssignMember {
            object: Box::new(Expression::This),
            property: Box::new(effective_property.clone()),
            value: Box::new(value.clone()),
        })
    }
}
