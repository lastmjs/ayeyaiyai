use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_arguments_descriptor_binding(
        &self,
        target: &Expression,
        string_property_name: Option<&str>,
    ) -> Option<PropertyDescriptorBinding> {
        if string_property_name == Some("length")
            && (self.is_direct_arguments_object(target)
                || self
                    .resolve_arguments_binding_from_expression(target)
                    .is_some())
        {
            if self.is_direct_arguments_object(target) {
                if !self
                    .state
                    .speculation
                    .execution_context
                    .current_arguments_length_present
                {
                    return None;
                }
            } else if !self
                .resolve_arguments_binding_from_expression(target)
                .is_some_and(|binding| binding.length_present)
            {
                return None;
            }
            return Some(PropertyDescriptorBinding {
                value: if self.is_direct_arguments_object(target) {
                    self.state
                        .speculation
                        .execution_context
                        .current_arguments_length_override
                        .clone()
                        .or(Some(Expression::Member {
                            object: Box::new(target.clone()),
                            property: Box::new(Expression::String("length".to_string())),
                        }))
                } else {
                    Some(
                        self.resolve_arguments_binding_from_expression(target)?
                            .length_value
                            .clone(),
                    )
                },
                configurable: true,
                enumerable: false,
                writable: Some(true),
                getter: None,
                setter: None,
                has_get: false,
                has_set: false,
            });
        }
        if let Some(index) =
            string_property_name.and_then(|property_name| property_name.parse::<usize>().ok())
            && (self.is_direct_arguments_object(target)
                || self
                    .resolve_arguments_binding_from_expression(target)
                    .is_some())
        {
            return Some(PropertyDescriptorBinding {
                value: if self.is_direct_arguments_object(target) {
                    self.state
                        .parameters
                        .arguments_slots
                        .get(&(index as u32))
                        .filter(|slot| slot.state.present)
                        .map(|_| Expression::Undefined)
                        .or(Some(Expression::Undefined))
                } else {
                    Some(
                        self.resolve_arguments_binding_from_expression(target)?
                            .values
                            .get(index)
                            .cloned()
                            .unwrap_or(Expression::Undefined),
                    )
                },
                configurable: true,
                enumerable: true,
                writable: Some(true),
                getter: None,
                setter: None,
                has_get: false,
                has_set: false,
            });
        }
        if string_property_name == Some("callee")
            && (self.is_direct_arguments_object(target)
                || self
                    .resolve_arguments_binding_from_expression(target)
                    .is_some())
        {
            let strict = if self.is_direct_arguments_object(target) {
                if !self
                    .state
                    .speculation
                    .execution_context
                    .current_arguments_callee_present
                {
                    return None;
                }
                self.state.speculation.execution_context.strict_mode
            } else {
                let binding = self.resolve_arguments_binding_from_expression(target)?;
                if !binding.callee_present {
                    return None;
                }
                binding.strict
            };
            return Some(if strict {
                PropertyDescriptorBinding {
                    value: None,
                    configurable: false,
                    enumerable: false,
                    writable: None,
                    getter: None,
                    setter: None,
                    has_get: true,
                    has_set: true,
                }
            } else {
                PropertyDescriptorBinding {
                    value: if self.is_direct_arguments_object(target) {
                        self.direct_arguments_callee_expression()
                    } else {
                        self.resolve_arguments_binding_from_expression(target)?
                            .callee_value
                            .clone()
                    },
                    configurable: true,
                    enumerable: false,
                    writable: Some(true),
                    getter: None,
                    setter: None,
                    has_get: false,
                    has_set: false,
                }
            });
        }
        None
    }
}
