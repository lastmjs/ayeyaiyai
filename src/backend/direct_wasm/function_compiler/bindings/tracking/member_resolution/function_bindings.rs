use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_member_function_binding(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> Option<LocalFunctionBinding> {
        let _guard = MemberFunctionBindingResolutionGuard::enter(object, property);
        if let Some(source_expression) = self.direct_iterator_binding_source_expression(object)
            && !std::ptr::eq(source_expression, object)
            && let Some(binding) = self.resolve_member_function_binding(source_expression, property)
        {
            return Some(binding);
        }
        let key = self.member_function_binding_key(object, property);
        let resolved = key
            .as_ref()
            .and_then(|key| self.member_function_binding_entry(key));
        if resolved.is_some() {
            return resolved;
        }
        for key in self.primitive_prototype_binding_keys(object, property) {
            if let Some(binding) = self.member_function_binding_entry(&key) {
                return Some(binding);
            }
        }

        if let Expression::Object(entries) = object
            && let Some(binding) = self.resolve_object_literal_member_binding(entries, property, 0)
        {
            return Some(binding);
        }
        let materialized_object = self.materialize_static_expression(object);
        let materialized_property = self.materialize_static_expression(property);
        if let (
            Expression::Member {
                object: prototype_owner,
                property: prototype_property,
            },
            Expression::String(property_name),
        ) = (&materialized_object, &materialized_property)
            && matches!(prototype_property.as_ref(), Expression::String(name) if name == "prototype")
            && let Expression::Identifier(object_name) = prototype_owner.as_ref()
            && self.is_unshadowed_builtin_identifier(object_name)
            && let Some(function_name) = builtin_prototype_function_name(object_name, property_name)
        {
            return Some(LocalFunctionBinding::Builtin(function_name.to_string()));
        }
        if let (Expression::Identifier(object_name), Expression::String(property_name)) =
            (&materialized_object, &materialized_property)
            && self.is_unshadowed_builtin_identifier(object_name)
            && let Some(function_name) = builtin_member_function_name(object_name, property_name)
        {
            return Some(LocalFunctionBinding::Builtin(function_name.to_string()));
        }
        let resolved = match object {
            Expression::Identifier(name) => {
                if let Some(index) = argument_index_from_expression(&materialized_property) {
                    if let Some(binding) = self
                        .state
                        .speculation
                        .static_semantics
                        .tracked_array_specialized_function_value(name, index)
                        .map(|value| value.binding.clone())
                    {
                        return Some(binding);
                    }
                    if let Some(value) = self
                        .state
                        .speculation
                        .static_semantics
                        .local_array_binding(name)
                        .or_else(|| self.global_array_binding(name))
                        .and_then(|array_binding| array_binding.values.get(index as usize))
                        .cloned()
                        .flatten()
                    {
                        return self.resolve_function_binding_from_expression(&value);
                    }
                }
                self.state
                    .speculation
                    .static_semantics
                    .local_object_binding(name)
                    .or_else(|| self.global_object_binding(name))
                    .and_then(|object_binding| {
                        object_binding_lookup_value(object_binding, &materialized_property)
                    })
                    .and_then(|value| self.resolve_function_binding_from_expression(value))
                    .or_else(|| {
                        self.resolve_object_binding_from_expression(object)
                            .and_then(|object_binding| {
                                object_binding_lookup_value(&object_binding, &materialized_property)
                                    .cloned()
                            })
                            .and_then(|value| self.resolve_function_binding_from_expression(&value))
                    })
            }
            Expression::Member { object, property } if matches!(property.as_ref(), Expression::String(name) if name == "prototype") =>
            {
                let Expression::Identifier(name) = object.as_ref() else {
                    return None;
                };
                self.resolve_function_prototype_object_binding(name)
                    .as_ref()
                    .and_then(|object_binding| {
                        object_binding_lookup_value(object_binding, &materialized_property)
                    })
                    .and_then(|value| self.resolve_function_binding_from_expression(value))
            }
            Expression::New { callee, .. } => {
                let Expression::Identifier(name) = callee.as_ref() else {
                    return None;
                };
                self.resolve_function_prototype_object_binding(name)
                    .as_ref()
                    .and_then(|object_binding| {
                        object_binding_lookup_value(object_binding, &materialized_property)
                    })
                    .and_then(|value| self.resolve_function_binding_from_expression(value))
            }
            _ => self
                .resolve_object_binding_from_expression(object)
                .and_then(|object_binding| {
                    object_binding_lookup_value(&object_binding, &materialized_property).cloned()
                })
                .and_then(|value| self.resolve_function_binding_from_expression(&value)),
        };
        if resolved.is_some() {
            return resolved;
        }

        if let Some(prototype) = self.resolve_static_object_prototype_expression(object)
            && !static_expression_matches(&prototype, object)
            && let Some(binding) =
                self.resolve_member_function_binding(&prototype, &materialized_property)
        {
            return Some(binding);
        }

        if !static_expression_matches(&materialized_object, object)
            || !static_expression_matches(&materialized_property, property)
        {
            return self
                .resolve_member_function_binding(&materialized_object, &materialized_property);
        }
        None
    }

    pub(in crate::backend::direct_wasm) fn resolve_member_function_capture_slots(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> Option<BTreeMap<String, String>> {
        if let Some(source_expression) = self.direct_iterator_binding_source_expression(object)
            && !std::ptr::eq(source_expression, object)
            && let Some(capture_slots) =
                self.resolve_member_function_capture_slots(source_expression, property)
        {
            return Some(capture_slots);
        }
        let key = self.member_function_binding_key(object, property)?;
        self.member_function_capture_slots_entry(&key)
    }

    pub(in crate::backend::direct_wasm) fn resolve_capture_slot_source_binding_name(
        &self,
        slot_name: &str,
    ) -> Option<String> {
        self.state
            .speculation
            .static_semantics
            .capture_slot_source_bindings
            .get(slot_name)
            .cloned()
            .or_else(|| {
                self.state
                    .speculation
                    .static_semantics
                    .local_value_binding(slot_name)
                    .and_then(|value| {
                        let Expression::Identifier(name) =
                            self.materialize_static_expression(value)
                        else {
                            return None;
                        };
                        Some(name)
                    })
            })
    }

    pub(in crate::backend::direct_wasm) fn resolve_member_function_capture_source_bindings(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> HashSet<String> {
        let mut names = HashSet::new();
        if let Some(capture_slots) = self.resolve_member_function_capture_slots(object, property) {
            for slot_name in capture_slots.values() {
                if let Some(name) = self.resolve_capture_slot_source_binding_name(slot_name) {
                    names.insert(name);
                }
            }
        }
        names
    }
}
