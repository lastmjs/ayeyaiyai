use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_object_property_descriptor_binding(
        &self,
        target: &Expression,
        resolved_target: Option<&Expression>,
        materialized_target: &Expression,
        property: &Expression,
        string_property_name: Option<&str>,
    ) -> Option<PropertyDescriptorBinding> {
        let object_binding = self
            .resolve_object_binding_from_expression(target)
            .or_else(|| {
                resolved_target
                    .and_then(|resolved| self.resolve_object_binding_from_expression(resolved))
            })
            .or_else(|| {
                (!static_expression_matches(materialized_target, target))
                    .then(|| self.resolve_object_binding_from_expression(materialized_target))?
            });
        let property_present_in_binding = object_binding.as_ref().is_some_and(|binding| {
            self.resolve_object_binding_property_value(binding, property)
                .is_some()
        });
        let value = object_binding
            .as_ref()
            .and_then(|binding| self.resolve_object_binding_property_value(binding, property));
        let getter = self
            .resolve_member_getter_binding(target, property)
            .or_else(|| {
                resolved_target
                    .and_then(|resolved| self.resolve_member_getter_binding(resolved, property))
            })
            .or_else(|| {
                (!static_expression_matches(materialized_target, target))
                    .then(|| self.resolve_member_getter_binding(materialized_target, property))?
            })
            .map(|binding| Self::function_binding_to_expression(&binding));
        let setter = self
            .resolve_member_setter_binding(target, property)
            .or_else(|| {
                resolved_target
                    .and_then(|resolved| self.resolve_member_setter_binding(resolved, property))
            })
            .or_else(|| {
                (!static_expression_matches(materialized_target, target))
                    .then(|| self.resolve_member_setter_binding(materialized_target, property))?
            })
            .map(|binding| Self::function_binding_to_expression(&binding));
        let enumerable = object_binding.as_ref().is_some_and(|binding| {
            property_present_in_binding
                && string_property_name.is_none_or(|property_name| {
                    !binding
                        .non_enumerable_string_properties
                        .iter()
                        .any(|name| name == property_name)
                })
        });
        if value.is_some() || getter.is_some() || setter.is_some() {
            return Some(PropertyDescriptorBinding {
                value: if getter.is_some() || setter.is_some() {
                    None
                } else {
                    value
                },
                configurable: true,
                enumerable,
                writable: if getter.is_some() || setter.is_some() {
                    None
                } else {
                    Some(true)
                },
                getter: getter.clone(),
                setter: setter.clone(),
                has_get: getter.is_some(),
                has_set: setter.is_some(),
            });
        }
        None
    }
}
