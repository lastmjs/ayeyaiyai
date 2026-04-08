use super::*;

impl<'a> FunctionCompiler<'a> {
    fn object_literal_member_function_bindings(
        &self,
        entries: &[crate::ir::hir::ObjectEntry],
    ) -> Vec<ReturnedMemberFunctionBinding> {
        entries
            .iter()
            .filter_map(|entry| {
                let crate::ir::hir::ObjectEntry::Data { key, value } = entry else {
                    return None;
                };
                let Expression::String(property) = key else {
                    return None;
                };
                let binding = self.resolve_function_binding_from_expression(value)?;
                Some(ReturnedMemberFunctionBinding {
                    target: ReturnedMemberFunctionBindingTarget::Value,
                    property: property.clone(),
                    binding,
                })
            })
            .collect()
    }

    fn object_literal_member_getter_bindings(
        &self,
        entries: &[crate::ir::hir::ObjectEntry],
    ) -> Vec<ReturnedMemberFunctionBinding> {
        entries
            .iter()
            .filter_map(|entry| {
                let crate::ir::hir::ObjectEntry::Getter { key, getter } = entry else {
                    return None;
                };
                let Expression::String(property) = key else {
                    return None;
                };
                let binding = self.resolve_function_binding_from_expression(getter)?;
                Some(ReturnedMemberFunctionBinding {
                    target: ReturnedMemberFunctionBindingTarget::Value,
                    property: property.clone(),
                    binding,
                })
            })
            .collect()
    }

    pub(in crate::backend::direct_wasm) fn inherited_member_function_bindings(
        &self,
        value: &Expression,
    ) -> Vec<ReturnedMemberFunctionBinding> {
        match value {
            Expression::Identifier(source_name) => {
                let local_bindings = self
                    .state
                    .speculation
                    .static_semantics
                    .objects
                    .member_function_bindings
                    .iter()
                    .map(|(key, binding)| (key.clone(), binding.clone()));
                let global_bindings = self.backend.global_member_function_binding_entries();
                local_bindings
                    .chain(global_bindings)
                    .filter_map(|(key, binding)| match &key.target {
                        MemberFunctionBindingTarget::Identifier(target)
                            if target.as_str() == source_name.as_str() =>
                        {
                            let MemberFunctionBindingProperty::String(property) = &key.property
                            else {
                                return None;
                            };
                            Some(ReturnedMemberFunctionBinding {
                                target: ReturnedMemberFunctionBindingTarget::Value,
                                property: property.clone(),
                                binding,
                            })
                        }
                        MemberFunctionBindingTarget::Prototype(target)
                            if target.as_str() == source_name.as_str() =>
                        {
                            let MemberFunctionBindingProperty::String(property) = &key.property
                            else {
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
                    .collect()
            }
            Expression::Call { callee, arguments } | Expression::New { callee, arguments } => {
                if let Some(object_binding) =
                    self.resolve_returned_object_binding_from_call(callee, arguments)
                {
                    let bindings = object_binding
                        .string_properties
                        .iter()
                        .filter_map(|(property, value)| {
                            let binding = self.resolve_function_binding_from_expression(value)?;
                            Some(ReturnedMemberFunctionBinding {
                                target: ReturnedMemberFunctionBindingTarget::Value,
                                property: property.clone(),
                                binding,
                            })
                        })
                        .collect::<Vec<_>>();
                    if !bindings.is_empty() {
                        return bindings;
                    }
                }
                let Some(user_function) = self.resolve_user_function_from_expression(callee) else {
                    return Vec::new();
                };
                user_function.returned_member_function_bindings.clone()
            }
            Expression::Object(entries) => self.object_literal_member_function_bindings(entries),
            _ => Vec::new(),
        }
    }

    pub(in crate::backend::direct_wasm) fn inherited_member_getter_bindings(
        &self,
        value: &Expression,
    ) -> Vec<ReturnedMemberFunctionBinding> {
        match value {
            Expression::Identifier(source_name) => {
                let local_bindings = self
                    .state
                    .speculation
                    .static_semantics
                    .objects
                    .member_getter_bindings
                    .iter()
                    .map(|(key, binding)| (key.clone(), binding.clone()));
                let global_bindings = self.backend.global_member_getter_binding_entries();
                local_bindings
                    .chain(global_bindings)
                    .filter_map(|(key, binding)| match &key.target {
                        MemberFunctionBindingTarget::Identifier(target)
                            if target.as_str() == source_name.as_str() =>
                        {
                            let MemberFunctionBindingProperty::String(property) = &key.property
                            else {
                                return None;
                            };
                            Some(ReturnedMemberFunctionBinding {
                                target: ReturnedMemberFunctionBindingTarget::Value,
                                property: property.clone(),
                                binding,
                            })
                        }
                        MemberFunctionBindingTarget::Prototype(target)
                            if target.as_str() == source_name.as_str() =>
                        {
                            let MemberFunctionBindingProperty::String(property) = &key.property
                            else {
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
                    .collect()
            }
            Expression::Call { callee, arguments } | Expression::New { callee, arguments } => self
                .resolve_static_call_result_expression_with_context(
                    callee,
                    arguments,
                    self.current_function_name(),
                )
                .map(|(value, _)| value)
                .and_then(|value| match value {
                    Expression::Object(entries) => {
                        Some(self.object_literal_member_getter_bindings(&entries))
                    }
                    _ => None,
                })
                .unwrap_or_default(),
            Expression::Object(entries) => self.object_literal_member_getter_bindings(entries),
            _ => Vec::new(),
        }
    }
}
