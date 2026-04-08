use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn update_static_global_assignment_metadata(
        &mut self,
        name: &str,
        value: &Expression,
    ) {
        let snapshot_value = self
            .global_value_binding(name)
            .map(|snapshot| substitute_self_referential_binding_snapshot(value, name, snapshot))
            .unwrap_or_else(|| value.clone());
        let mut referenced_names = HashSet::new();
        collect_referenced_binding_names_from_expression(&snapshot_value, &mut referenced_names);
        if referenced_names.contains(name) {
            self.clear_global_binding_state(name);
            return;
        }
        self.set_global_binding_kind(name, infer_global_expression_kind(&snapshot_value));
        let materialized_value = self.materialize_global_expression(&snapshot_value);
        let inferred_array_binding = self.infer_global_array_binding(&snapshot_value);
        let inferred_object_binding = self.infer_global_object_binding(&snapshot_value);
        let inferred_arguments_binding = self.infer_global_arguments_binding(&snapshot_value);
        let inferred_function_binding = self.infer_global_function_binding(&snapshot_value);
        self.set_global_expression_binding(name, materialized_value);
        self.sync_global_array_binding(name, inferred_array_binding);
        self.sync_global_object_binding(name, inferred_object_binding);
        self.sync_global_arguments_binding(name, inferred_arguments_binding);
        self.sync_global_function_binding(name, inferred_function_binding);
        let materialized_snapshot = self.materialize_global_expression(&snapshot_value);
        if let Expression::Identifier(source_name) = &materialized_snapshot {
            self.copy_global_member_bindings_for_alias(name, source_name);
        } else {
            let preserved_capture_slots =
                self.global_member_capture_slots_by_property_for_name(name);
            let inherited_member_bindings =
                self.global_inherited_member_function_bindings(&snapshot_value);
            let inherited_getter_bindings =
                self.global_inherited_member_getter_bindings(&snapshot_value);
            if inherited_member_bindings.is_empty() && inherited_getter_bindings.is_empty() {
                if !self.has_global_member_bindings_for_name(name) {
                    self.update_global_object_literal_member_bindings_for_value(
                        name,
                        &snapshot_value,
                    );
                }
            } else {
                self.clear_global_member_bindings_for_name(name);
                for binding in inherited_member_bindings {
                    self.insert_global_inherited_member_function_binding_for_name(
                        name,
                        binding,
                        &preserved_capture_slots,
                    );
                }
                for binding in inherited_getter_bindings {
                    self.insert_global_inherited_member_getter_binding_for_name(
                        name,
                        binding,
                        &preserved_capture_slots,
                    );
                }
            }
        }
        self.update_global_object_literal_home_bindings(name, &snapshot_value);
        self.update_global_object_prototype_binding_from_value(name, &snapshot_value);
    }

    pub(in crate::backend::direct_wasm) fn update_global_member_assignment_metadata(
        &mut self,
        object: &Expression,
        property: &Expression,
        value: &Expression,
    ) {
        let materialized_property = self.materialize_global_expression(property);
        let materialized_value = self.materialize_global_expression(value);
        if let Expression::Identifier(name) = object
            && let Some(index) =
                argument_index_from_expression(&materialized_property).map(|index| index as usize)
            && self.set_global_array_element_binding(name, index, materialized_value.clone())
        {
        }
        if let Expression::Identifier(name) = object
            && matches!(&materialized_property, Expression::String(property_name) if property_name == "prototype")
            && let Some(prototype) = self.prototype_assignment_parent_expression(value)
        {
            self.update_global_object_prototype_binding(&format!("{name}.prototype"), &prototype);
        }
        match object {
            Expression::Identifier(name) if self.global_has_binding(name) => {
                self.define_global_object_property(
                    name,
                    materialized_property.clone(),
                    materialized_value.clone(),
                    true,
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
                self.define_global_prototype_object_property(
                    name,
                    materialized_property.clone(),
                    materialized_value.clone(),
                    true,
                );
            }
            _ => {}
        }

        let Some(key) = self.global_member_function_binding_key(object, property) else {
            return;
        };
        if let Some(binding) = self.infer_global_function_binding(value) {
            self.set_global_member_function_binding(key.clone(), binding);
        } else {
            self.clear_global_member_function_binding(&key);
        }
        self.clear_global_member_getter_binding(&key);
        self.clear_global_member_setter_binding(&key);
    }
}
