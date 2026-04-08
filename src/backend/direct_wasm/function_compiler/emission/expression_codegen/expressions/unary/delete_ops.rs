use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_delete_expression(
        &mut self,
        expression: &Expression,
    ) -> DirectResult<()> {
        match expression {
            Expression::Identifier(name)
                if self.resolve_current_local_binding(name).is_none()
                    && self.backend.global_binding_index(name).is_none()
                    && self.resolve_eval_local_function_hidden_name(name).is_some() =>
            {
                self.clear_eval_local_function_binding_metadata(name);
                self.emit_delete_eval_local_function_binding(name)?;
                return Ok(());
            }
            Expression::Identifier(name)
                if self.resolve_current_local_binding(name).is_none()
                    && self.backend.global_binding_index(name).is_none()
                    && self.backend.global_has_implicit_binding(name) =>
            {
                self.state
                    .runtime
                    .locals
                    .deleted_builtin_identifiers
                    .remove(name);
                self.emit_delete_implicit_global_binding(name)?;
                return Ok(());
            }
            Expression::Identifier(name)
                if self.resolve_current_local_binding(name).is_none()
                    && self.backend.global_binding_index(name).is_none()
                    && self.is_unshadowed_builtin_identifier(name)
                    && builtin_identifier_delete_returns_true(name) =>
            {
                self.clear_static_identifier_binding_metadata(name);
                self.state
                    .runtime
                    .locals
                    .deleted_builtin_identifiers
                    .insert(name.clone());
                self.push_i32_const(1);
                return Ok(());
            }
            Expression::Identifier(name) if self.is_identifier_bound(name) => {
                self.push_i32_const(0);
            }
            Expression::Identifier(_) => {
                self.push_i32_const(1);
            }
            Expression::Member { object, property } if matches!(property.as_ref(), Expression::String(property_name) if property_name == "callee" || property_name == "length") =>
            {
                let Expression::String(property_name) = property.as_ref() else {
                    unreachable!("filtered above");
                };
                if self.is_direct_arguments_object(object) {
                    match property_name.as_str() {
                        "callee" => {
                            if self.state.speculation.execution_context.strict_mode {
                                self.push_i32_const(0);
                            } else {
                                self.apply_current_arguments_effect(
                                    "callee",
                                    ArgumentsPropertyEffect::Delete,
                                );
                                self.push_i32_const(1);
                            }
                        }
                        "length" => {
                            self.apply_current_arguments_effect(
                                "length",
                                ArgumentsPropertyEffect::Delete,
                            );
                            self.push_i32_const(1);
                        }
                        _ => unreachable!("filtered above"),
                    }
                    self.emit_delete_result_or_throw_if_strict()?;
                    return Ok(());
                }
                if let Some(arguments_binding) =
                    self.resolve_arguments_binding_from_expression(object)
                {
                    self.emit_numeric_expression(object)?;
                    self.state.emission.output.instructions.push(0x1a);
                    self.emit_numeric_expression(property)?;
                    self.state.emission.output.instructions.push(0x1a);
                    if property_name == "callee" && arguments_binding.strict {
                        self.push_i32_const(0);
                    } else {
                        self.update_named_arguments_binding_effect(
                            object,
                            property_name,
                            ArgumentsPropertyEffect::Delete,
                        );
                        self.push_i32_const(1);
                    }
                    return Ok(());
                }
                if property_name == "length"
                    && self.resolve_array_binding_from_expression(object).is_some()
                {
                    self.push_i32_const(0);
                    return Ok(());
                }
                self.emit_numeric_expression(expression)?;
                self.state.emission.output.instructions.push(0x1a);
                self.push_i32_const(1);
            }
            Expression::Member { object, property }
                if self.is_direct_arguments_object(object)
                    && argument_index_from_expression(property).is_some() =>
            {
                self.emit_arguments_slot_delete(
                    argument_index_from_expression(property).expect("checked above"),
                );
                self.emit_delete_result_or_throw_if_strict()?;
                return Ok(());
            }
            Expression::Member { object, property }
                if argument_index_from_expression(property).is_some() =>
            {
                let index = argument_index_from_expression(property).expect("checked above");
                if let Expression::Identifier(name) = object.as_ref() {
                    if let Some(array_binding) = self
                        .state
                        .speculation
                        .static_semantics
                        .local_array_binding_mut(name)
                    {
                        if let Some(value) = array_binding.values.get_mut(index as usize) {
                            *value = None;
                        }
                        self.clear_runtime_array_slot(name, index);
                        self.push_i32_const(1);
                        return Ok(());
                    }
                    if let Some(array_binding) = self
                        .backend
                        .global_semantics
                        .values
                        .array_bindings
                        .get_mut(name)
                    {
                        if let Some(value) = array_binding.values.get_mut(index as usize) {
                            *value = None;
                        }
                        self.clear_global_runtime_array_slot(name, index);
                        self.push_i32_const(1);
                        return Ok(());
                    }
                    if let Some(arguments_binding) =
                        self.state.parameters.local_arguments_bindings.get_mut(name)
                    {
                        if let Some(value) = arguments_binding.values.get_mut(index as usize) {
                            *value = Expression::Undefined;
                        }
                        self.push_i32_const(1);
                        return Ok(());
                    }
                    if let Some(arguments_binding) = self
                        .backend
                        .global_semantics
                        .values
                        .arguments_bindings
                        .get_mut(name)
                    {
                        if let Some(value) = arguments_binding.values.get_mut(index as usize) {
                            *value = Expression::Undefined;
                        }
                        self.push_i32_const(1);
                        return Ok(());
                    }
                }
                self.emit_numeric_expression(expression)?;
                self.state.emission.output.instructions.push(0x1a);
                self.push_i32_const(1);
            }
            Expression::Member { object, property } => {
                let resolved_property = self
                    .resolve_property_key_expression(property)
                    .unwrap_or_else(|| self.materialize_static_expression(property));
                if matches!(
                    resolved_property,
                    Expression::String(ref property_name) if property_name == "length"
                ) && self.resolve_array_binding_from_expression(object).is_some()
                {
                    self.push_i32_const(0);
                    return Ok(());
                }
                if let (Expression::Identifier(object_name), Expression::String(property_name)) = (
                    self.materialize_static_expression(object),
                    resolved_property.clone(),
                ) && self.is_unshadowed_builtin_identifier(&object_name)
                    && builtin_member_delete_returns_false(&object_name, &property_name)
                {
                    self.push_i32_const(0);
                    return Ok(());
                }
                if let Expression::Identifier(name) = object.as_ref() {
                    let materialized_property = resolved_property;
                    self.clear_runtime_object_property_shadow_binding(
                        object,
                        &materialized_property,
                    );
                    if let Some(object_binding) = self
                        .state
                        .speculation
                        .static_semantics
                        .local_object_binding_mut(name)
                    {
                        object_binding_remove_property(object_binding, &materialized_property);
                        self.push_i32_const(1);
                        return Ok(());
                    }
                    if let Some(object_binding) = self
                        .backend
                        .global_semantics
                        .values
                        .object_bindings
                        .get_mut(name)
                    {
                        object_binding_remove_property(object_binding, &materialized_property);
                        self.push_i32_const(1);
                        return Ok(());
                    }
                }
                self.emit_numeric_expression(expression)?;
                self.state.emission.output.instructions.push(0x1a);
                self.push_i32_const(1);
            }
            Expression::SuperMember { .. }
            | Expression::AssignMember { .. }
            | Expression::AssignSuperMember { .. }
            | Expression::This => {
                self.emit_numeric_expression(expression)?;
                self.state.emission.output.instructions.push(0x1a);
                self.push_i32_const(1);
            }
            _ => {
                self.emit_numeric_expression(expression)?;
                self.state.emission.output.instructions.push(0x1a);
                self.push_i32_const(1);
            }
        }
        Ok(())
    }
}
