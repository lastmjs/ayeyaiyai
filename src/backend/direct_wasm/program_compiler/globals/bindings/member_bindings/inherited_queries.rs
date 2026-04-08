use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn global_inherited_member_function_bindings(
        &self,
        value: &Expression,
    ) -> Vec<ReturnedMemberFunctionBinding> {
        match value {
            Expression::Identifier(source_name) => self
                .global_member_function_binding_entries()
                .into_iter()
                .filter_map(|(key, binding)| match &key.target {
                    MemberFunctionBindingTarget::Identifier(target) if target == source_name => {
                        let MemberFunctionBindingProperty::String(property) = &key.property else {
                            return None;
                        };
                        Some(ReturnedMemberFunctionBinding {
                            target: ReturnedMemberFunctionBindingTarget::Value,
                            property: property.clone(),
                            binding,
                        })
                    }
                    MemberFunctionBindingTarget::Prototype(target) if target == source_name => {
                        let MemberFunctionBindingProperty::String(property) = &key.property else {
                            return None;
                        };
                        Some(ReturnedMemberFunctionBinding {
                            target: ReturnedMemberFunctionBindingTarget::Prototype,
                            property: property.clone(),
                            binding,
                        })
                    }
                    _ => None,
                })
                .collect(),
            Expression::Call { callee, .. } | Expression::New { callee, .. } => {
                let Some(LocalFunctionBinding::User(function_name)) =
                    self.infer_global_function_binding(callee)
                else {
                    return Vec::new();
                };
                self.user_function_returned_member_function_bindings(&function_name)
            }
            _ => Vec::new(),
        }
    }

    pub(in crate::backend::direct_wasm) fn global_inherited_member_getter_bindings(
        &self,
        value: &Expression,
    ) -> Vec<ReturnedMemberFunctionBinding> {
        match value {
            Expression::Identifier(source_name) => self
                .global_member_getter_binding_entries()
                .into_iter()
                .filter_map(|(key, binding)| match &key.target {
                    MemberFunctionBindingTarget::Identifier(target) if target == source_name => {
                        let MemberFunctionBindingProperty::String(property) = &key.property else {
                            return None;
                        };
                        Some(ReturnedMemberFunctionBinding {
                            target: ReturnedMemberFunctionBindingTarget::Value,
                            property: property.clone(),
                            binding,
                        })
                    }
                    MemberFunctionBindingTarget::Prototype(target) if target == source_name => {
                        let MemberFunctionBindingProperty::String(property) = &key.property else {
                            return None;
                        };
                        Some(ReturnedMemberFunctionBinding {
                            target: ReturnedMemberFunctionBindingTarget::Prototype,
                            property: property.clone(),
                            binding,
                        })
                    }
                    _ => None,
                })
                .collect(),
            Expression::Call { callee, arguments } | Expression::New { callee, arguments } => self
                .infer_static_call_result_expression(callee, arguments)
                .and_then(|value| match value {
                    Expression::Object(entries) => Some(
                        entries
                            .into_iter()
                            .filter_map(|entry| match entry {
                                ObjectEntry::Getter { key, getter } => {
                                    let Expression::String(property) = key else {
                                        return None;
                                    };
                                    let binding = self.infer_global_function_binding(&getter)?;
                                    Some(ReturnedMemberFunctionBinding {
                                        target: ReturnedMemberFunctionBindingTarget::Value,
                                        property,
                                        binding,
                                    })
                                }
                                _ => None,
                            })
                            .collect(),
                    ),
                    _ => None,
                })
                .unwrap_or_default(),
            _ => Vec::new(),
        }
    }

    pub(in crate::backend::direct_wasm) fn global_member_capture_slots_by_property_for_name(
        &self,
        name: &str,
    ) -> HashMap<String, BTreeMap<String, String>> {
        self.global_member_function_capture_slot_entries()
            .into_iter()
            .filter_map(|(key, capture_slots)| {
                let property_name = match (&key.target, &key.property) {
                    (
                        MemberFunctionBindingTarget::Identifier(target)
                        | MemberFunctionBindingTarget::Prototype(target),
                        MemberFunctionBindingProperty::String(property),
                    ) if target == name => Some(property.clone()),
                    _ => None,
                }?;
                Some((property_name, capture_slots))
            })
            .collect()
    }

    pub(in crate::backend::direct_wasm) fn has_global_member_bindings_for_name(
        &self,
        name: &str,
    ) -> bool {
        self.global_member_function_binding_entries()
            .into_iter()
            .map(|(key, _)| key)
            .any(|key| {
                matches!(
                    key.target,
                    MemberFunctionBindingTarget::Identifier(target)
                        | MemberFunctionBindingTarget::Prototype(target)
                        if target == name
                )
            })
            || self
                .global_member_getter_binding_entries()
                .into_iter()
                .map(|(key, _)| key)
                .any(|key| {
                    matches!(
                        key.target,
                        MemberFunctionBindingTarget::Identifier(target)
                            | MemberFunctionBindingTarget::Prototype(target)
                            if target == name
                    )
                })
            || self
                .global_member_setter_binding_entries()
                .into_iter()
                .map(|(key, _)| key)
                .any(|key| {
                    matches!(
                        key.target,
                        MemberFunctionBindingTarget::Identifier(target)
                            | MemberFunctionBindingTarget::Prototype(target)
                            if target == name
                    )
                })
            || self
                .global_member_function_capture_slot_entries()
                .into_iter()
                .map(|(key, _)| key)
                .any(|key| {
                    matches!(
                        key.target,
                        MemberFunctionBindingTarget::Identifier(target)
                            | MemberFunctionBindingTarget::Prototype(target)
                            if target == name
                    )
                })
    }
}
