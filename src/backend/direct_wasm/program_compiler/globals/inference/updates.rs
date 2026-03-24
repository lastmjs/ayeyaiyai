use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn infer_global_function_binding(
        &self,
        expression: &Expression,
    ) -> Option<LocalFunctionBinding> {
        match expression {
            Expression::Identifier(name) => {
                if let Some(binding) = self.global_function_bindings.get(name) {
                    return Some(binding.clone());
                }
                if is_internal_user_function_identifier(name)
                    && self.user_function_map.contains_key(name)
                {
                    Some(LocalFunctionBinding::User(name.clone()))
                } else if builtin_identifier_kind(name) == Some(StaticValueKind::Function) {
                    Some(LocalFunctionBinding::Builtin(name.clone()))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn update_static_global_assignment_metadata(
        &mut self,
        name: &str,
        value: &Expression,
    ) {
        let snapshot_value = self
            .global_value_bindings
            .get(name)
            .map(|snapshot| substitute_self_referential_binding_snapshot(value, name, snapshot))
            .unwrap_or_else(|| value.clone());
        let mut referenced_names = HashSet::new();
        collect_referenced_binding_names_from_expression(&snapshot_value, &mut referenced_names);
        if referenced_names.contains(name) {
            self.global_value_bindings.remove(name);
            self.global_array_bindings.remove(name);
            self.global_object_bindings.remove(name);
            self.global_arguments_bindings.remove(name);
            self.global_function_bindings.remove(name);
            self.global_kinds.remove(name);
            return;
        }
        self.global_kinds.insert(
            name.to_string(),
            infer_global_expression_kind(&snapshot_value),
        );
        let materialized_value = self.materialize_global_expression(&snapshot_value);
        self.global_value_bindings
            .insert(name.to_string(), materialized_value);
        if let Some(array_binding) = self.infer_global_array_binding(&snapshot_value) {
            self.global_array_bindings
                .insert(name.to_string(), array_binding);
        } else {
            self.global_array_bindings.remove(name);
        }
        if let Some(object_binding) = self.infer_global_object_binding(&snapshot_value) {
            self.global_object_bindings
                .insert(name.to_string(), object_binding);
        } else {
            self.global_object_bindings.remove(name);
        }
        if let Some(arguments_binding) = self.infer_global_arguments_binding(&snapshot_value) {
            self.global_arguments_bindings
                .insert(name.to_string(), arguments_binding);
        } else {
            self.global_arguments_bindings.remove(name);
        }
        if let Some(function_binding) = self.infer_global_function_binding(&snapshot_value) {
            self.global_function_bindings
                .insert(name.to_string(), function_binding);
            self.global_kinds
                .insert(name.to_string(), StaticValueKind::Function);
        } else {
            self.global_function_bindings.remove(name);
        }
        let materialized_snapshot = self.materialize_global_expression(&snapshot_value);
        if let Expression::Identifier(source_name) = &materialized_snapshot {
            self.copy_global_member_bindings_for_alias(name, source_name);
        } else {
            let inherited_member_bindings =
                self.global_inherited_member_function_bindings(&snapshot_value);
            if inherited_member_bindings.is_empty() {
                self.update_global_object_literal_member_bindings_for_value(name, &snapshot_value);
            } else {
                self.clear_global_member_bindings_for_name(name);
                for binding in inherited_member_bindings {
                    self.insert_global_inherited_member_function_binding_for_name(name, binding);
                }
            }
        }
        self.update_global_object_literal_home_bindings(name, &snapshot_value);
        self.update_global_object_prototype_binding_from_value(name, &snapshot_value);
    }

    pub(in crate::backend::direct_wasm) fn global_member_function_binding_property(
        &self,
        property: &Expression,
    ) -> Option<MemberFunctionBindingProperty> {
        let materialized = self.materialize_global_expression(property);
        if let Some(property_name) = static_property_name_from_expression(&materialized) {
            return Some(MemberFunctionBindingProperty::String(property_name));
        }
        match &materialized {
            Expression::Member { object, property }
                if matches!(object.as_ref(), Expression::Identifier(name) if name == "Symbol")
                    && matches!(property.as_ref(), Expression::String(_)) =>
            {
                let Expression::String(symbol_name) = property.as_ref() else {
                    unreachable!("filtered above");
                };
                Some(MemberFunctionBindingProperty::Symbol(format!(
                    "Symbol.{symbol_name}"
                )))
            }
            Expression::Call { callee, .. } if matches!(callee.as_ref(), Expression::Identifier(name) if name == "Symbol") => {
                Some(MemberFunctionBindingProperty::SymbolExpression(format!(
                    "{materialized:?}"
                )))
            }
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn global_member_function_binding_key(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> Option<MemberFunctionBindingKey> {
        let target = match object {
            Expression::Identifier(name) => MemberFunctionBindingTarget::Identifier(name.clone()),
            Expression::Member {
                object,
                property: target_property,
            } if matches!(target_property.as_ref(), Expression::String(name) if name == "prototype") =>
            {
                let Expression::Identifier(name) = object.as_ref() else {
                    return None;
                };
                MemberFunctionBindingTarget::Prototype(name.clone())
            }
            _ => return None,
        };
        let property = self.global_member_function_binding_property(property)?;
        Some(MemberFunctionBindingKey { target, property })
    }

    pub(in crate::backend::direct_wasm) fn update_global_member_assignment_metadata(
        &mut self,
        object: &Expression,
        property: &Expression,
        value: &Expression,
    ) {
        let materialized_property = self.materialize_global_expression(property);
        let materialized_value = self.materialize_global_expression(value);
        match object {
            Expression::Identifier(name) if self.global_bindings.contains_key(name) => {
                let object_binding = self
                    .global_object_bindings
                    .entry(name.clone())
                    .or_insert_with(empty_object_value_binding);
                object_binding_set_property(
                    object_binding,
                    materialized_property.clone(),
                    materialized_value.clone(),
                );
            }
            Expression::Member {
                object: prototype_object,
                property: target_property,
            } if matches!(target_property.as_ref(), Expression::String(name) if name == "prototype") =>
            {
                let Expression::Identifier(name) = prototype_object.as_ref() else {
                    return;
                };
                let object_binding = self
                    .global_prototype_object_bindings
                    .entry(name.clone())
                    .or_insert_with(empty_object_value_binding);
                object_binding_set_property(
                    object_binding,
                    materialized_property.clone(),
                    materialized_value.clone(),
                );
            }
            _ => {}
        }

        let Some(key) = self.global_member_function_binding_key(object, property) else {
            return;
        };
        if let Some(binding) = self.infer_global_function_binding(value) {
            self.global_member_function_bindings
                .insert(key.clone(), binding);
        } else {
            self.global_member_function_bindings.remove(&key);
        }
        self.global_member_getter_bindings.remove(&key);
        self.global_member_setter_bindings.remove(&key);
    }
}
