use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_scoped_property_store_from_local(
        &mut self,
        scope_object: &Expression,
        name: &str,
        value_local: u32,
        value_expression: &Expression,
    ) -> DirectResult<()> {
        let property = Expression::String(name.to_string());
        if let Some(binding) =
            self.resolve_runtime_object_property_shadow_binding(scope_object, &property)
        {
            self.push_local_get(value_local);
            self.push_global_set(binding.value_index);
            self.push_i32_const(1);
            self.push_global_set(binding.present_index);
        }
        if let Some(setter_binding) = self.resolve_member_setter_binding(scope_object, &property) {
            let receiver_hidden_name = self.allocate_named_hidden_local(
                "scoped_setter_receiver",
                self.infer_value_kind(scope_object)
                    .unwrap_or(StaticValueKind::Unknown),
            );
            let receiver_local = self
                .state
                .runtime
                .locals
                .get(&receiver_hidden_name)
                .copied()
                .expect("fresh scoped setter receiver hidden local must exist");
            let value_hidden_name = self.allocate_named_hidden_local(
                "scoped_setter_value",
                self.infer_value_kind(value_expression)
                    .unwrap_or(StaticValueKind::Unknown),
            );
            let value_hidden_local = self
                .state
                .runtime
                .locals
                .get(&value_hidden_name)
                .copied()
                .expect("fresh scoped setter value hidden local must exist");
            self.emit_numeric_expression(scope_object)?;
            self.push_local_set(receiver_local);
            self.push_local_get(value_local);
            self.push_local_set(value_hidden_local);
            self.update_local_value_binding(&receiver_hidden_name, scope_object);
            self.update_local_object_binding(&receiver_hidden_name, scope_object);
            self.update_capture_slot_binding_from_expression(&value_hidden_name, value_expression)?;
            let receiver_expression = Expression::Identifier(receiver_hidden_name);
            if self.emit_function_binding_call_with_function_this_binding_from_argument_locals(
                &setter_binding,
                &[value_hidden_local],
                1,
                &receiver_expression,
            )? {
                self.state.emission.output.instructions.push(0x1a);
            }
            self.push_local_get(value_local);
            return Ok(());
        }

        let materialized_value = self.materialize_static_expression(value_expression);
        if let Expression::Identifier(scope_name) = scope_object {
            if let Some(object_binding) = self
                .state
                .speculation
                .static_semantics
                .local_object_binding_mut(scope_name)
            {
                object_binding_set_property(
                    object_binding,
                    property.clone(),
                    materialized_value.clone(),
                );
                self.push_local_get(value_local);
                return Ok(());
            }
            if let Some(object_binding) = self
                .backend
                .global_semantics
                .values
                .object_binding_mut(scope_name)
            {
                object_binding_set_property(object_binding, property, materialized_value);
                self.push_local_get(value_local);
                return Ok(());
            }
            if let Some(hidden_name) = self.resolve_user_function_capture_hidden_name(scope_name) {
                if let Some(object_binding) = self
                    .backend
                    .global_semantics
                    .values
                    .object_binding_mut(&hidden_name)
                {
                    object_binding_set_property(object_binding, property, materialized_value);
                    self.push_local_get(value_local);
                    return Ok(());
                }
            }
        }

        self.push_local_get(value_local);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_scoped_property_update(
        &mut self,
        scope_object: &Expression,
        name: &str,
        op: UpdateOp,
        prefix: bool,
    ) -> DirectResult<()> {
        let opcode = match op {
            UpdateOp::Increment => 0x6a,
            UpdateOp::Decrement => 0x6b,
        };
        let property = Expression::String(name.to_string());
        let member_expression = Expression::Member {
            object: Box::new(scope_object.clone()),
            property: Box::new(property.clone()),
        };
        let previous_kind = self
            .infer_value_kind(&member_expression)
            .unwrap_or(StaticValueKind::Unknown);
        let current_value = self
            .resolve_object_binding_from_expression(scope_object)
            .and_then(|object_binding| {
                object_binding_lookup_value(&object_binding, &property).cloned()
            })
            .unwrap_or(Expression::Undefined);
        let increment = match op {
            UpdateOp::Increment => 1.0,
            UpdateOp::Decrement => -1.0,
        };

        match previous_kind {
            StaticValueKind::Undefined
            | StaticValueKind::String
            | StaticValueKind::Object
            | StaticValueKind::Function
            | StaticValueKind::Symbol
            | StaticValueKind::BigInt => {
                let nan_local = self.allocate_temp_local();
                self.push_i32_const(JS_NAN_TAG);
                self.push_local_set(nan_local);
                self.emit_scoped_property_store_from_local(
                    scope_object,
                    name,
                    nan_local,
                    &Expression::Number(f64::NAN),
                )?;
                self.state.emission.output.instructions.push(0x1a);
                self.push_local_get(nan_local);
                return Ok(());
            }
            StaticValueKind::Null => {
                let previous_local = self.allocate_temp_local();
                let next_local = self.allocate_temp_local();
                self.push_i32_const(0);
                self.push_local_set(previous_local);
                self.push_i32_const(increment as i32);
                self.push_local_set(next_local);
                self.emit_scoped_property_store_from_local(
                    scope_object,
                    name,
                    next_local,
                    &Expression::Number(increment),
                )?;
                self.state.emission.output.instructions.push(0x1a);
                if prefix {
                    self.push_local_get(next_local);
                } else {
                    self.push_local_get(previous_local);
                }
                return Ok(());
            }
            _ => {}
        }

        let previous_local = self.allocate_temp_local();
        let next_local = self.allocate_temp_local();
        self.emit_scoped_property_read(scope_object, name)?;
        self.push_local_set(previous_local);
        self.push_local_get(previous_local);
        self.push_i32_const(1);
        self.state.emission.output.instructions.push(opcode);
        self.push_local_set(next_local);
        let next_expression = match previous_kind {
            StaticValueKind::Bool => {
                let previous = match self.materialize_static_expression(&current_value) {
                    Expression::Bool(value) => {
                        if value {
                            1.0
                        } else {
                            0.0
                        }
                    }
                    _ => 0.0,
                };
                Expression::Number(previous + increment)
            }
            _ => self
                .resolve_static_number_value(&current_value)
                .map(|value| Expression::Number(value + increment))
                .unwrap_or(Expression::Number(f64::NAN)),
        };
        self.emit_scoped_property_store_from_local(
            scope_object,
            name,
            next_local,
            &next_expression,
        )?;
        self.state.emission.output.instructions.push(0x1a);
        if prefix {
            self.push_local_get(next_local);
        } else {
            self.push_local_get(previous_local);
        }
        Ok(())
    }
}
