use super::*;

impl DirectWasmCompiler {
    pub(super) fn update_parameter_binding_state_from_define_property_call(
        &self,
        expression: &Expression,
        value_bindings: &mut HashMap<String, Expression>,
        object_bindings: &mut HashMap<String, ObjectValueBinding>,
    ) {
        let Expression::Call { callee, arguments } = expression else {
            return;
        };
        let Expression::Member { object, property } = callee.as_ref() else {
            return;
        };
        if !matches!(object.as_ref(), Expression::Identifier(name) if name == "Object")
            || !matches!(property.as_ref(), Expression::String(name) if name == "defineProperty")
        {
            return;
        }
        let [
            CallArgument::Expression(target),
            CallArgument::Expression(property),
            CallArgument::Expression(descriptor_expression),
            ..,
        ] = arguments.as_slice()
        else {
            return;
        };
        let Some(descriptor) = resolve_property_descriptor_definition(descriptor_expression) else {
            return;
        };
        let Expression::Identifier(name) = target else {
            return;
        };

        let property =
            self.materialize_callback_state_expression(property, value_bindings, object_bindings);
        let property_name = static_property_name_from_expression(&property);
        let existing_value = object_bindings
            .get(name)
            .and_then(|object_binding| object_binding_lookup_value(object_binding, &property))
            .cloned();
        let current_enumerable = property_name.as_ref().is_some_and(|property_name| {
            object_bindings
                .get(name)
                .map(|object_binding| {
                    !object_binding
                        .non_enumerable_string_properties
                        .iter()
                        .any(|hidden_name| hidden_name == property_name)
                })
                .unwrap_or(false)
        });
        let enumerable = descriptor.enumerable.unwrap_or(current_enumerable);
        let value = if descriptor.is_accessor() {
            Expression::Undefined
        } else {
            descriptor
                .value
                .as_ref()
                .map(|expression| {
                    self.materialize_callback_state_expression(
                        expression,
                        value_bindings,
                        object_bindings,
                    )
                })
                .or(existing_value)
                .unwrap_or(Expression::Undefined)
        };
        let object_binding = object_bindings
            .entry(name.clone())
            .or_insert_with(empty_object_value_binding);
        object_binding_define_property(object_binding, property, value, enumerable);
    }
}
