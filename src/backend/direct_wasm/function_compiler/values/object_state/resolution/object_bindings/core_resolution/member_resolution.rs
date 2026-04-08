use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn resolve_member_object_binding(
        &self,
        expression: &Expression,
    ) -> Option<ObjectValueBinding> {
        let Expression::Member { object, property } = expression else {
            return None;
        };
        if matches!(property.as_ref(), Expression::String(name) if name == "global") {
            let realm_id = self.resolve_test262_realm_id_from_expression(object)?;
            return self.test262_realm_global_object_binding(realm_id);
        }
        if matches!(property.as_ref(), Expression::String(name) if name == "prototype") {
            let Expression::Identifier(name) = object.as_ref() else {
                return None;
            };
            return self.resolve_function_prototype_object_binding(name);
        }

        let property = self
            .resolve_property_key_expression(property)
            .unwrap_or_else(|| self.materialize_static_expression(property));
        if let Some(IteratorStepBinding::Runtime {
            static_value: Some(value),
            ..
        }) = self.resolve_iterator_step_binding_from_expression(object)
            && matches!(property, Expression::String(ref name) if name == "value")
        {
            return self.resolve_object_binding_from_expression(&value);
        }
        if let Some(index) = argument_index_from_expression(&property)
            && let Some(array_binding) = self.resolve_array_binding_from_expression(object)
            && let Some(Some(value)) = array_binding.values.get(index as usize)
        {
            return self.resolve_object_binding_from_expression(value);
        }
        None
    }
}
