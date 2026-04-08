use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_setter_member_assignment(
        &mut self,
        object: &Expression,
        property: &Expression,
        value: &Expression,
    ) -> DirectResult<bool> {
        let Some(function_binding) = self.resolve_member_setter_binding(object, property) else {
            return Ok(false);
        };

        let receiver_hidden_name = self.allocate_named_hidden_local(
            "setter_receiver",
            self.infer_value_kind(object)
                .unwrap_or(StaticValueKind::Unknown),
        );
        let receiver_local = self
            .state
            .runtime
            .locals
            .get(&receiver_hidden_name)
            .copied()
            .expect("fresh setter receiver hidden local must exist");
        let value_hidden_name = self.allocate_named_hidden_local(
            "setter_value",
            self.infer_value_kind(value)
                .unwrap_or(StaticValueKind::Unknown),
        );
        let value_local = self
            .state
            .runtime
            .locals
            .get(&value_hidden_name)
            .copied()
            .expect("fresh setter value hidden local must exist");
        self.emit_numeric_expression(object)?;
        self.push_local_set(receiver_local);
        self.emit_numeric_expression(value)?;
        self.push_local_set(value_local);
        self.update_local_value_binding(&receiver_hidden_name, object);
        self.update_local_object_binding(&receiver_hidden_name, object);
        self.update_capture_slot_binding_from_expression(&value_hidden_name, value)?;
        let receiver_expression = Expression::Identifier(receiver_hidden_name.clone());
        if self.emit_function_binding_call_with_function_this_binding_from_argument_locals(
            &function_binding,
            &[value_local],
            1,
            &receiver_expression,
        )? {
            self.state.emission.output.instructions.push(0x1a);
        }
        if let Expression::Identifier(name) = object
            && let Some(updated_receiver) = self
                .state
                .speculation
                .static_semantics
                .last_bound_user_function_call
                .as_ref()
                .and_then(|snapshot| {
                    snapshot
                        .updated_bindings
                        .get(&receiver_hidden_name)
                        .cloned()
                })
        {
            self.update_local_value_binding(name, &updated_receiver);
            self.update_local_object_binding(name, &updated_receiver);
        }
        self.push_local_get(value_local);
        Ok(true)
    }
}
