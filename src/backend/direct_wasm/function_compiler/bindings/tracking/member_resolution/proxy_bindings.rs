use super::*;

impl<'a> FunctionCompiler<'a> {
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
                self.member_function_binding_entry(&key).or_else(|| {
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
                .state
                .speculation
                .static_semantics
                .local_proxy_binding(name)
                .cloned()
                .or_else(|| self.backend.global_proxy_binding(name).cloned()),
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
            self.state
                .speculation
                .static_semantics
                .clear_local_proxy_binding(name);
            if self.binding_name_is_global(name) {
                self.backend.sync_global_proxy_binding(name, None);
            }
            return;
        };
        self.state
            .speculation
            .static_semantics
            .set_local_proxy_binding(name, proxy_binding.clone());
        if self.binding_name_is_global(name) {
            self.backend
                .sync_global_proxy_binding(name, Some(proxy_binding));
        }
        self.state
            .speculation
            .static_semantics
            .set_local_kind(name, StaticValueKind::Object);
    }
}
