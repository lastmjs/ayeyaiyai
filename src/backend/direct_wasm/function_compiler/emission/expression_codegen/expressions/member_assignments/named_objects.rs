use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_named_object_member_assignment(
        &mut self,
        object: &Expression,
        property: &Expression,
        value: &Expression,
    ) -> DirectResult<bool> {
        if let Expression::Member {
            object: prototype_object,
            property: target_property,
        } = object
            && matches!(target_property.as_ref(), Expression::String(name) if name == "prototype")
        {
            let Expression::Identifier(name) = prototype_object.as_ref() else {
                unreachable!("filtered above");
            };
            let materialized_property = self.canonical_object_property_expression(property);
            let materialized = self.materialize_static_expression(value);
            if let Some(object_binding) = self
                .state
                .speculation
                .static_semantics
                .objects
                .local_prototype_object_bindings
                .get_mut(name)
            {
                object_binding_set_property(
                    object_binding,
                    materialized_property.clone(),
                    materialized.clone(),
                );
            }
            if self.binding_name_is_global(name) {
                let object_binding = self
                    .backend
                    .global_semantics
                    .values
                    .prototype_object_bindings
                    .entry(name.clone())
                    .or_insert_with(empty_object_value_binding);
                object_binding_set_property(object_binding, materialized_property, materialized);
            }
            self.update_member_function_assignment_binding(object, property, value);
            self.emit_numeric_expression(value)?;
            return Ok(true);
        }

        let Expression::Identifier(name) = object else {
            return Ok(false);
        };

        let static_array_property = if inline_summary_side_effect_free_expression(property)
            && !self.expression_depends_on_active_loop_assignment(property)
        {
            self.resolve_property_key_expression(property)
                .unwrap_or_else(|| self.materialize_static_expression(property))
        } else {
            property.clone()
        };

        if self
            .state
            .speculation
            .static_semantics
            .has_local_typed_array_view_binding(name)
        {
            self.emit_typed_array_view_write(name, property, value)?;
            return Ok(true);
        }
        if let Some(realm_id) = self.resolve_test262_realm_global_id_from_expression(object) {
            let materialized_property = self.canonical_object_property_expression(property);
            let materialized = self.materialize_static_expression(value);
            if let Some(realm) = self.test262_realm_mut(realm_id) {
                object_binding_set_property(
                    &mut realm.global_object_binding,
                    materialized_property,
                    materialized,
                );
                self.emit_numeric_expression(value)?;
                return Ok(true);
            }
        }
        if let Some(index) = argument_index_from_expression(&static_array_property) {
            let materialized = self.materialize_static_expression(value);
            let length_local = self
                .state
                .speculation
                .static_semantics
                .runtime_array_length_local(name);
            let use_global_runtime_array = self.is_named_global_array_binding(name)
                && (!self.state.speculation.execution_context.top_level_function
                    || self.uses_global_runtime_array_state(name));
            let value_local = self.allocate_temp_local();
            self.emit_numeric_expression(value)?;
            self.push_local_set(value_local);
            let mut array_length = None;
            if let Some(array_binding) = self
                .state
                .speculation
                .static_semantics
                .local_array_binding_mut(name)
            {
                while array_binding.values.len() <= index as usize {
                    array_binding.values.push(None);
                }
                array_binding.values[index as usize] = Some(materialized.clone());
                array_length = Some(array_binding.values.len() as i32);
            } else if let Some(array_binding) = self
                .backend
                .global_semantics
                .values
                .array_bindings
                .get_mut(name)
            {
                while array_binding.values.len() <= index as usize {
                    array_binding.values.push(None);
                }
                array_binding.values[index as usize] = Some(materialized);
                array_length = Some(array_binding.values.len() as i32);
            }
            if let Some(array_length) = array_length {
                self.update_tracked_array_specialized_function_value(name, index, value)?;
                if !use_global_runtime_array && let Some(length_local) = length_local {
                    self.push_i32_const(array_length);
                    self.push_local_set(length_local);
                }
                if use_global_runtime_array {
                    if self.emit_global_runtime_array_slot_write_from_local(
                        name,
                        index,
                        value_local,
                    )? {
                        self.state.emission.output.instructions.push(0x1a);
                    }
                } else if self.emit_runtime_array_slot_write_from_local(name, index, value_local)? {
                    self.state.emission.output.instructions.push(0x1a);
                }
                self.push_local_get(value_local);
                return Ok(true);
            }
        }
        if self.is_named_global_array_binding(name)
            && (!self.state.speculation.execution_context.top_level_function
                || self.uses_global_runtime_array_state(name))
        {
            if self.emit_dynamic_global_runtime_array_slot_write(name, property, value)? {
                return Ok(true);
            }
        } else if self.emit_dynamic_runtime_array_slot_write(name, property, value)? {
            return Ok(true);
        }
        let resolved_property = if self.expression_depends_on_active_loop_assignment(property) {
            self.materialize_static_expression(property)
        } else {
            self.resolve_property_key_expression(property)
                .unwrap_or_else(|| self.materialize_static_expression(property))
        };
        if self
            .state
            .speculation
            .static_semantics
            .has_local_array_binding(name)
            || self
                .backend
                .global_semantics
                .values
                .array_bindings
                .contains_key(name)
        {
            let materialized = self.materialize_static_expression(value);
            if self
                .state
                .speculation
                .static_semantics
                .has_local_array_binding(name)
            {
                let object_binding = self
                    .state
                    .speculation
                    .static_semantics
                    .ensure_local_object_binding(name);
                object_binding_set_property(
                    object_binding,
                    resolved_property.clone(),
                    materialized.clone(),
                );
            }
            if self
                .backend
                .global_semantics
                .values
                .array_bindings
                .contains_key(name)
            {
                let object_binding = self
                    .backend
                    .global_semantics
                    .values
                    .object_bindings
                    .entry(name.clone())
                    .or_insert_with(empty_object_value_binding);
                object_binding_set_property(
                    object_binding,
                    resolved_property.clone(),
                    materialized,
                );
            }
        }
        if let Expression::String(property_name) = resolved_property
            && self
                .runtime_object_property_shadow_owner_name_for_identifier(name)
                .is_some()
        {
            let value_local = self.allocate_temp_local();
            self.emit_numeric_expression(value)?;
            self.push_local_set(value_local);
            self.emit_scoped_property_store_from_local(object, &property_name, value_local, value)?;
            return Ok(true);
        }
        let materialized_property = self.canonical_object_property_expression(property);
        let materialized = self.materialize_static_expression(value);
        if let Some(object_binding) = self
            .state
            .speculation
            .static_semantics
            .local_object_binding_mut(name)
        {
            object_binding_set_property(
                object_binding,
                materialized_property.clone(),
                materialized.clone(),
            );
            self.update_member_function_assignment_binding(object, property, value);
            self.emit_numeric_expression(value)?;
            return Ok(true);
        }
        if let Some(object_binding) = self
            .backend
            .global_semantics
            .values
            .object_bindings
            .get_mut(name)
        {
            object_binding_set_property(object_binding, materialized_property, materialized);
            self.update_member_function_assignment_binding(object, property, value);
            self.emit_numeric_expression(value)?;
            return Ok(true);
        }
        if self
            .resolve_function_binding_from_expression(object)
            .is_some()
        {
            let object_binding = self
                .state
                .speculation
                .static_semantics
                .ensure_local_object_binding(name);
            object_binding_set_property(
                object_binding,
                materialized_property.clone(),
                materialized.clone(),
            );
            if self.binding_name_is_global(name) {
                let global_binding = self
                    .backend
                    .global_semantics
                    .values
                    .object_bindings
                    .entry(name.clone())
                    .or_insert_with(empty_object_value_binding);
                object_binding_set_property(global_binding, materialized_property, materialized);
            }
            self.state
                .speculation
                .static_semantics
                .set_local_kind(name, StaticValueKind::Object);
            self.update_member_function_assignment_binding(object, property, value);
            self.emit_numeric_expression(value)?;
            return Ok(true);
        }

        Ok(false)
    }
}
