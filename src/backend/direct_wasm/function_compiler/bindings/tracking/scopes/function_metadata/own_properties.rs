use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn function_object_has_explicit_own_property(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> bool {
        let resolved_object = self
            .resolve_bound_alias_expression(object)
            .filter(|resolved| !static_expression_matches(resolved, object));
        let materialized_object = self.materialize_static_expression(object);
        let resolved_property = self.resolve_property_key_expression(property).or_else(|| {
            self.resolve_bound_alias_expression(property)
                .filter(|resolved| !static_expression_matches(resolved, property))
        });
        let materialized_property = self.materialize_static_expression(property);

        let object_candidates = [
            Some(object),
            resolved_object.as_ref(),
            (!static_expression_matches(&materialized_object, object))
                .then_some(&materialized_object),
        ];
        let property_candidates = [
            Some(property),
            resolved_property.as_ref(),
            (!static_expression_matches(&materialized_property, property))
                .then_some(&materialized_property),
        ];

        object_candidates
            .into_iter()
            .flatten()
            .any(|object_candidate| {
                property_candidates
                    .into_iter()
                    .flatten()
                    .any(|property_candidate| {
                        self.resolve_member_function_binding(object_candidate, property_candidate)
                            .is_some()
                            || self
                                .resolve_member_getter_binding(object_candidate, property_candidate)
                                .is_some()
                            || self
                                .resolve_member_setter_binding(object_candidate, property_candidate)
                                .is_some()
                            || self
                                .resolve_object_binding_from_expression(object_candidate)
                                .is_some_and(|object_binding| {
                                    self.resolve_object_binding_property_value(
                                        &object_binding,
                                        property_candidate,
                                    )
                                    .is_some()
                                })
                    })
            })
    }
}
