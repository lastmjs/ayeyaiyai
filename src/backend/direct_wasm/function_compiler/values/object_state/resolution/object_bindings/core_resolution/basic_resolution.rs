use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn resolve_basic_object_binding(
        &self,
        expression: &Expression,
    ) -> Option<ObjectValueBinding> {
        match expression {
            Expression::Identifier(name) => {
                self.resolve_identifier_object_binding(name, expression)
            }
            Expression::This => self.resolve_this_object_binding(),
            _ => None,
        }
    }

    fn resolve_identifier_object_binding(
        &self,
        name: &str,
        expression: &Expression,
    ) -> Option<ObjectValueBinding> {
        let resolved_name = self
            .resolve_current_local_binding(name)
            .map(|(resolved_name, _)| resolved_name)
            .unwrap_or_else(|| name.to_string());
        if name == "$262" {
            let mut host_object_binding = empty_object_value_binding();
            object_binding_set_property(
                &mut host_object_binding,
                Expression::String("createRealm".to_string()),
                Expression::Identifier(TEST262_CREATE_REALM_BUILTIN.to_string()),
            );
            return Some(host_object_binding);
        }
        if let Some(realm_id) = parse_test262_realm_identifier(name) {
            return self.backend.test262_realm_object_binding(realm_id);
        }
        if let Some(realm_id) = parse_test262_realm_global_identifier(name) {
            return self.test262_realm_global_object_binding(realm_id);
        }
        self.state
            .speculation
            .static_semantics
            .local_object_binding(&resolved_name)
            .cloned()
            .or_else(|| {
                let hidden_name = self.resolve_user_function_capture_hidden_name(name)?;
                self.global_object_binding(&hidden_name).cloned()
            })
            .or_else(|| self.global_object_binding(name).cloned())
            .or_else(|| {
                let proxy = self
                    .state
                    .speculation
                    .static_semantics
                    .local_proxy_binding(&resolved_name)
                    .cloned()
                    .or_else(|| self.global_proxy_binding(name).cloned())?;
                self.resolve_object_binding_from_expression(&proxy.target)
            })
            .or_else(|| {
                let resolved = self.resolve_bound_alias_expression(expression)?;
                (!static_expression_matches(&resolved, expression))
                    .then(|| self.resolve_object_binding_from_expression(&resolved))
                    .flatten()
            })
    }

    fn resolve_this_object_binding(&self) -> Option<ObjectValueBinding> {
        self.state
            .speculation
            .static_semantics
            .local_object_binding("this")
            .cloned()
            .or_else(|| {
                self.state
                    .speculation
                    .static_semantics
                    .local_value_binding("this")
                    .cloned()
                    .and_then(|value| {
                        (!matches!(value, Expression::Undefined))
                            .then(|| self.resolve_object_binding_from_expression(&value))
                            .flatten()
                    })
            })
    }
}
