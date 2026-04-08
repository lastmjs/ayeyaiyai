use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn member_function_binding_entry(
        &self,
        key: &MemberFunctionBindingKey,
    ) -> Option<LocalFunctionBinding> {
        self.state
            .speculation
            .static_semantics
            .objects
            .member_function_bindings
            .get(key)
            .cloned()
            .or_else(|| self.backend.global_member_function_binding(key).cloned())
    }

    pub(super) fn member_function_capture_slots_entry(
        &self,
        key: &MemberFunctionBindingKey,
    ) -> Option<BTreeMap<String, String>> {
        self.state
            .speculation
            .static_semantics
            .objects
            .member_function_capture_slots
            .get(key)
            .cloned()
            .or_else(|| {
                self.backend
                    .global_member_function_capture_slots(key)
                    .cloned()
            })
    }

    pub(super) fn member_getter_binding_entry(
        &self,
        key: &MemberFunctionBindingKey,
    ) -> Option<LocalFunctionBinding> {
        self.state
            .speculation
            .static_semantics
            .objects
            .member_getter_bindings
            .get(key)
            .cloned()
            .or_else(|| self.backend.global_member_getter_binding(key).cloned())
    }

    pub(super) fn member_setter_binding_entry(
        &self,
        key: &MemberFunctionBindingKey,
    ) -> Option<LocalFunctionBinding> {
        self.state
            .speculation
            .static_semantics
            .objects
            .member_setter_bindings
            .get(key)
            .cloned()
            .or_else(|| self.backend.global_member_setter_binding(key).cloned())
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
}
