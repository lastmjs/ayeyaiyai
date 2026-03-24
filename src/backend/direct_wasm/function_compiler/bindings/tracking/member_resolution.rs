use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_direct_iterator_step_member_read(
        &mut self,
        object: &Expression,
        property: &Expression,
    ) -> DirectResult<bool> {
        let Expression::String(property_name) = property else {
            return Ok(false);
        };
        if property_name != "done" && property_name != "value" {
            return Ok(false);
        }
        let Expression::Call { callee, arguments } = object else {
            return Ok(false);
        };
        if !arguments.is_empty() {
            return Ok(false);
        }
        let Expression::Member {
            object: iterator_object,
            property: next_property,
        } = callee.as_ref()
        else {
            return Ok(false);
        };
        if !matches!(next_property.as_ref(), Expression::String(name) if name == "next") {
            return Ok(false);
        }
        let hidden_name =
            self.allocate_named_hidden_local("direct_iterator_step", StaticValueKind::Object);
        self.update_local_iterator_step_binding(&hidden_name, object);
        let Some(IteratorStepBinding::Runtime {
            done_local,
            value_local,
            ..
        }) = self.local_iterator_step_bindings.get(&hidden_name).cloned()
        else {
            return Ok(false);
        };
        self.emit_numeric_expression(iterator_object)?;
        self.instructions.push(0x1a);
        match property_name.as_str() {
            "done" => self.push_local_get(done_local),
            "value" => self.push_local_get(value_local),
            _ => unreachable!("filtered above"),
        }
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn resolve_object_literal_member_binding(
        &self,
        entries: &[crate::ir::hir::ObjectEntry],
        property: &Expression,
        slot: u8,
    ) -> Option<LocalFunctionBinding> {
        let materialized_property = self.materialize_static_expression(property);
        let target_property = self.member_function_binding_property(&materialized_property)?;
        let mut state = (None, None, None);

        for entry in entries {
            let (key, binding, entry_slot) = match entry {
                crate::ir::hir::ObjectEntry::Data { key, value } => {
                    (key, self.resolve_function_binding_from_expression(value), 0)
                }
                crate::ir::hir::ObjectEntry::Getter { key, getter } => (
                    key,
                    self.resolve_function_binding_from_expression(getter),
                    1,
                ),
                crate::ir::hir::ObjectEntry::Setter { key, setter } => (
                    key,
                    self.resolve_function_binding_from_expression(setter),
                    2,
                ),
                crate::ir::hir::ObjectEntry::Spread(_) => return None,
            };
            let materialized_key = self
                .resolve_property_key_expression(key)
                .unwrap_or_else(|| self.materialize_static_expression(key));
            let Some(property_name) = self.member_function_binding_property(&materialized_key)
            else {
                continue;
            };
            if property_name != target_property {
                continue;
            }
            match entry_slot {
                0 => {
                    state.0 = binding;
                    state.1 = None;
                    state.2 = None;
                }
                1 => {
                    state.0 = None;
                    state.1 = binding;
                }
                2 => {
                    state.0 = None;
                    state.2 = binding;
                }
                _ => {}
            }
        }

        match slot {
            0 => state.0,
            1 => state.1,
            2 => state.2,
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn direct_iterator_binding_source_expression<'b>(
        &self,
        value: &'b Expression,
    ) -> Option<&'b Expression> {
        let iterated = match value {
            Expression::GetIterator(iterated) => iterated.as_ref(),
            Expression::Call { callee, arguments }
                if arguments.is_empty()
                    && matches!(
                        callee.as_ref(),
                        Expression::Member { property, .. }
                            if is_symbol_iterator_expression(property)
                    ) =>
            {
                let Expression::Member { object, .. } = callee.as_ref() else {
                    unreachable!("filtered above");
                };
                object.as_ref()
            }
            _ => return None,
        };
        let next_property = Expression::String("next".to_string());
        let has_next_binding = self
            .resolve_member_function_binding(iterated, &next_property)
            .is_some();
        let has_iterator_source_kind = self.resolve_iterator_source_kind(iterated).is_some();
        let has_next_property = self
            .resolve_object_binding_from_expression(iterated)
            .is_some_and(|object_binding| {
                object_binding_has_property(&object_binding, &next_property)
            });
        if has_next_binding || has_iterator_source_kind || has_next_property {
            return Some(iterated);
        }
        None
    }

    pub(in crate::backend::direct_wasm) fn with_suspended_with_scopes<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> DirectResult<T>,
    ) -> DirectResult<T> {
        let previous_with_scopes = std::mem::take(&mut self.with_scopes);
        let result = f(self);
        self.with_scopes = previous_with_scopes;
        result
    }

    pub(in crate::backend::direct_wasm) fn primitive_prototype_binding_keys(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> Vec<MemberFunctionBindingKey> {
        let Some(binding_property) = self.member_function_binding_property(property) else {
            return Vec::new();
        };
        let materialized_object = self.materialize_static_expression(object);
        let materialized_property = self
            .resolve_property_key_expression(property)
            .unwrap_or_else(|| self.materialize_static_expression(property));
        let target_names = match &materialized_object {
            Expression::Number(_) => vec!["Number", "Object"],
            Expression::String(_) => {
                if matches!(
                    materialized_property,
                    Expression::String(ref name) if name == "length"
                ) || argument_index_from_expression(&materialized_property).is_some()
                {
                    Vec::new()
                } else {
                    vec!["String", "Object"]
                }
            }
            Expression::Bool(_) => vec!["Boolean", "Object"],
            Expression::BigInt(_) => vec!["BigInt", "Object"],
            Expression::Identifier(name)
                if self.lookup_identifier_kind(name) == Some(StaticValueKind::Symbol) =>
            {
                vec!["Symbol", "Object"]
            }
            _ => Vec::new(),
        };
        target_names
            .into_iter()
            .map(|target| MemberFunctionBindingKey {
                target: MemberFunctionBindingTarget::Prototype(target.to_string()),
                property: binding_property.clone(),
            })
            .collect()
    }

    pub(in crate::backend::direct_wasm) fn resolve_member_function_binding(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> Option<LocalFunctionBinding> {
        if let Some(source_expression) = self.direct_iterator_binding_source_expression(object)
            && !std::ptr::eq(source_expression, object)
            && let Some(binding) = self.resolve_member_function_binding(source_expression, property)
        {
            return Some(binding);
        }
        let key = self.member_function_binding_key(object, property);
        let resolved = key.and_then(|key| {
            self.member_function_bindings
                .get(&key)
                .cloned()
                .or_else(|| {
                    self.module
                        .global_member_function_bindings
                        .get(&key)
                        .cloned()
                })
        });
        if resolved.is_some() {
            return resolved;
        }
        for key in self.primitive_prototype_binding_keys(object, property) {
            if let Some(binding) = self
                .member_function_bindings
                .get(&key)
                .cloned()
                .or_else(|| {
                    self.module
                        .global_member_function_bindings
                        .get(&key)
                        .cloned()
                })
            {
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
                        .tracked_array_function_values
                        .get(name)
                        .and_then(|bindings| bindings.get(&index))
                        .map(|value| value.binding.clone())
                    {
                        return Some(binding);
                    }
                    if let Some(value) = self
                        .local_array_bindings
                        .get(name)
                        .or_else(|| self.module.global_array_bindings.get(name))
                        .and_then(|array_binding| array_binding.values.get(index as usize))
                        .cloned()
                        .flatten()
                    {
                        return self.resolve_function_binding_from_expression(&value);
                    }
                }
                self.local_object_bindings
                    .get(name)
                    .or_else(|| self.module.global_object_bindings.get(name))
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
        let resolved = self
            .member_function_capture_slots
            .get(&key)
            .cloned()
            .or_else(|| {
                self.module
                    .global_member_function_capture_slots
                    .get(&key)
                    .cloned()
            });
        resolved
    }

    pub(in crate::backend::direct_wasm) fn resolve_capture_slot_source_binding_name(
        &self,
        slot_name: &str,
    ) -> Option<String> {
        self.capture_slot_source_bindings
            .get(slot_name)
            .cloned()
            .or_else(|| {
                self.local_value_bindings.get(slot_name).and_then(|value| {
                    let Expression::Identifier(name) = self.materialize_static_expression(value)
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

    pub(in crate::backend::direct_wasm) fn resolve_member_getter_binding(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> Option<LocalFunctionBinding> {
        let key = self.member_function_binding_key(object, property);
        let resolved = key.and_then(|key| {
            self.member_getter_bindings
                .get(&key)
                .cloned()
                .or_else(|| self.module.global_member_getter_bindings.get(&key).cloned())
        });
        if resolved.is_some() {
            return resolved;
        }
        for key in self.primitive_prototype_binding_keys(object, property) {
            if let Some(binding) = self
                .member_getter_bindings
                .get(&key)
                .cloned()
                .or_else(|| self.module.global_member_getter_bindings.get(&key).cloned())
            {
                return Some(binding);
            }
        }

        if let Expression::Object(entries) = object
            && let Some(binding) = self.resolve_object_literal_member_binding(entries, property, 1)
        {
            return Some(binding);
        }

        let materialized_object = self.materialize_static_expression(object);
        let materialized_property = self.materialize_static_expression(property);
        if let Some(prototype) = self.resolve_static_object_prototype_expression(object)
            && !static_expression_matches(&prototype, object)
            && let Some(binding) =
                self.resolve_member_getter_binding(&prototype, &materialized_property)
        {
            return Some(binding);
        }
        if !static_expression_matches(&materialized_object, object)
            || !static_expression_matches(&materialized_property, property)
        {
            return self
                .resolve_member_getter_binding(&materialized_object, &materialized_property);
        }
        None
    }

    pub(in crate::backend::direct_wasm) fn resolve_member_setter_binding(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> Option<LocalFunctionBinding> {
        let key = self.member_function_binding_key(object, property);
        let resolved = key.and_then(|key| {
            self.member_setter_bindings
                .get(&key)
                .cloned()
                .or_else(|| self.module.global_member_setter_bindings.get(&key).cloned())
        });
        if resolved.is_some() {
            return resolved;
        }
        for key in self.primitive_prototype_binding_keys(object, property) {
            if let Some(binding) = self
                .member_setter_bindings
                .get(&key)
                .cloned()
                .or_else(|| self.module.global_member_setter_bindings.get(&key).cloned())
            {
                return Some(binding);
            }
        }

        if let Expression::Object(entries) = object
            && let Some(binding) = self.resolve_object_literal_member_binding(entries, property, 2)
        {
            return Some(binding);
        }

        let materialized_object = self.materialize_static_expression(object);
        let materialized_property = self.materialize_static_expression(property);
        if let Some(prototype) = self.resolve_static_object_prototype_expression(object)
            && !static_expression_matches(&prototype, object)
            && let Some(binding) =
                self.resolve_member_setter_binding(&prototype, &materialized_property)
        {
            return Some(binding);
        }

        if !static_expression_matches(&materialized_object, object)
            || !static_expression_matches(&materialized_property, property)
        {
            return self
                .resolve_member_setter_binding(&materialized_object, &materialized_property);
        }
        None
    }

    pub(in crate::backend::direct_wasm) fn resolve_proxy_has_binding_from_handler(
        &self,
        handler: &Expression,
    ) -> Option<LocalFunctionBinding> {
        let property = Expression::String("has".to_string());
        match handler {
            Expression::Identifier(name) => {
                let key = MemberFunctionBindingKey {
                    target: MemberFunctionBindingTarget::Identifier(name.clone()),
                    property: MemberFunctionBindingProperty::String("has".to_string()),
                };
                self.member_function_bindings
                    .get(&key)
                    .cloned()
                    .or_else(|| {
                        self.module
                            .global_member_function_bindings
                            .get(&key)
                            .cloned()
                    })
                    .or_else(|| {
                        self.resolve_object_binding_from_expression(handler)
                            .and_then(|object_binding| {
                                object_binding_lookup_value(&object_binding, &property).and_then(
                                    |value| self.resolve_function_binding_from_expression(value),
                                )
                            })
                    })
            }
            Expression::Object(entries) => entries.iter().find_map(|entry| {
                let crate::ir::hir::ObjectEntry::Data { key, value } = entry else {
                    return None;
                };
                let key = self
                    .resolve_property_key_expression(key)
                    .unwrap_or_else(|| self.materialize_static_expression(key));
                if !matches!(key, Expression::String(ref name) if name == "has") {
                    return None;
                }
                self.resolve_function_binding_from_expression(value)
            }),
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_proxy_binding_from_expression(
        &self,
        expression: &Expression,
    ) -> Option<ProxyValueBinding> {
        match expression {
            Expression::Identifier(name) => self
                .local_proxy_bindings
                .get(name)
                .cloned()
                .or_else(|| self.module.global_proxy_bindings.get(name).cloned()),
            Expression::New { callee, arguments } if matches!(callee.as_ref(), Expression::Identifier(name) if name == "Proxy" && self.is_unshadowed_builtin_identifier(name)) =>
            {
                let [
                    CallArgument::Expression(target),
                    CallArgument::Expression(handler),
                    ..,
                ] = arguments.as_slice()
                else {
                    return None;
                };
                Some(ProxyValueBinding {
                    target: self.materialize_static_expression(target),
                    has_binding: self.resolve_proxy_has_binding_from_handler(handler),
                })
            }
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn update_local_proxy_binding(
        &mut self,
        name: &str,
        value: &Expression,
    ) {
        let Some(proxy_binding) = self.resolve_proxy_binding_from_expression(value) else {
            self.local_proxy_bindings.remove(name);
            if self.binding_name_is_global(name) {
                self.module.global_proxy_bindings.remove(name);
            }
            return;
        };
        self.local_proxy_bindings
            .insert(name.to_string(), proxy_binding.clone());
        if self.binding_name_is_global(name) {
            self.module
                .global_proxy_bindings
                .insert(name.to_string(), proxy_binding);
        }
        self.local_kinds
            .insert(name.to_string(), StaticValueKind::Object);
    }
}
