use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_member_getter_binding(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> Option<LocalFunctionBinding> {
        let key = self.member_function_binding_key(object, property);
        let resolved = key
            .as_ref()
            .and_then(|key| self.member_getter_binding_entry(key));
        if resolved.is_some() {
            return resolved;
        }
        for key in self.primitive_prototype_binding_keys(object, property) {
            if let Some(binding) = self.member_getter_binding_entry(&key) {
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

    pub(in crate::backend::direct_wasm) fn resolve_member_getter_binding_shallow(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> Option<LocalFunctionBinding> {
        let key = self.member_function_binding_key(object, property);
        let resolved = key
            .as_ref()
            .and_then(|key| self.member_getter_binding_entry(key));
        if resolved.is_some() {
            return resolved;
        }
        if let Expression::Object(entries) = object {
            return self.resolve_object_literal_member_binding(entries, property, 1);
        }
        None
    }

    pub(in crate::backend::direct_wasm) fn resolve_member_setter_binding(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> Option<LocalFunctionBinding> {
        let key = self.member_function_binding_key(object, property);
        let resolved = key
            .as_ref()
            .and_then(|key| self.member_setter_binding_entry(key));
        if resolved.is_some() {
            return resolved;
        }
        for key in self.primitive_prototype_binding_keys(object, property) {
            if let Some(binding) = self.member_setter_binding_entry(&key) {
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
}
