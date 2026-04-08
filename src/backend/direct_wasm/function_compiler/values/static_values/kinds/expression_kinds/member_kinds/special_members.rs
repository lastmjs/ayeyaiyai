use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn infer_special_member_kind(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> Option<StaticValueKind> {
        if let Some(step_binding) = self.resolve_iterator_step_binding_from_expression(object)
            && let Expression::String(property_name) = property
        {
            return match (property_name.as_str(), step_binding) {
                ("done", _) => Some(StaticValueKind::Bool),
                (
                    "value",
                    IteratorStepBinding::Runtime {
                        function_binding, ..
                    },
                ) => function_binding
                    .as_ref()
                    .and_then(|binding| match binding {
                        LocalFunctionBinding::User(function_name) => self
                            .user_function(function_name)
                            .map(|_| StaticValueKind::Function),
                        LocalFunctionBinding::Builtin(_) => Some(StaticValueKind::Function),
                    })
                    .or(Some(StaticValueKind::Unknown)),
                _ => Some(StaticValueKind::Unknown),
            };
        }
        if let Expression::Call { callee, arguments } = object
            && arguments.is_empty()
            && let Expression::Member {
                object: iterator_object,
                property: next_property,
            } = callee.as_ref()
            && matches!(next_property.as_ref(), Expression::String(name) if name == "next")
            && matches!(iterator_object.as_ref(), Expression::Identifier(name) if self.resolve_local_array_iterator_binding_name(name).is_some())
            && matches!(property, Expression::String(name) if name == "done")
        {
            return Some(StaticValueKind::Bool);
        }
        if let Expression::Identifier(name) = object
            && let resolved_name = self
                .resolve_current_local_binding(name)
                .map(|(resolved_name, _)| resolved_name)
                .unwrap_or_else(|| name.clone())
            && let Some(descriptor) = self
                .state
                .speculation
                .static_semantics
                .objects
                .local_descriptor_bindings
                .get(&resolved_name)
            && let Expression::String(property_name) = property
        {
            return match property_name.as_str() {
                "value" => descriptor
                    .value
                    .as_ref()
                    .and_then(|value| self.infer_value_kind(value))
                    .or(Some(StaticValueKind::Undefined)),
                "configurable" | "enumerable" => Some(StaticValueKind::Bool),
                "writable" => {
                    if descriptor.writable.is_some() {
                        Some(StaticValueKind::Bool)
                    } else {
                        Some(StaticValueKind::Undefined)
                    }
                }
                "get" => descriptor
                    .getter
                    .as_ref()
                    .and_then(|getter| self.infer_value_kind(getter))
                    .or_else(|| {
                        if descriptor.has_get {
                            Some(StaticValueKind::Function)
                        } else {
                            Some(StaticValueKind::Undefined)
                        }
                    }),
                "set" => descriptor
                    .setter
                    .as_ref()
                    .and_then(|setter| self.infer_value_kind(setter))
                    .or_else(|| {
                        if descriptor.has_set {
                            Some(StaticValueKind::Function)
                        } else {
                            Some(StaticValueKind::Undefined)
                        }
                    }),
                _ => Some(StaticValueKind::Unknown),
            };
        }
        None
    }
}
