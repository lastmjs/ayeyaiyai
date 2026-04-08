use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn apply_object_define_property_update(&mut self, arguments: &[CallArgument]) {
        let [
            CallArgument::Expression(target),
            CallArgument::Expression(property),
            CallArgument::Expression(descriptor_expression),
            ..,
        ] = arguments
        else {
            return;
        };
        let Some(descriptor) = resolve_property_descriptor_definition(descriptor_expression) else {
            return;
        };

        match target {
            Expression::This => {
                self.update_global_property_descriptor(property, &descriptor);
            }
            Expression::Identifier(name) => {
                self.define_object_property_from_descriptor(name, property, &descriptor);
            }
            Expression::Member {
                object,
                property: target_property,
            } if matches!(target_property.as_ref(), Expression::String(name) if name == "prototype") =>
            {
                let Expression::Identifier(name) = object.as_ref() else {
                    return;
                };
                self.define_prototype_object_property_from_descriptor(name, property, &descriptor);
            }
            _ => {}
        }
    }

    fn update_global_property_descriptor(
        &mut self,
        property: &Expression,
        descriptor: &PropertyDescriptorDefinition,
    ) {
        let property = self
            .resolve_property_key_expression(property)
            .unwrap_or_else(|| self.materialize_static_expression(property));
        let property_name: String = match static_property_name_from_expression(&property) {
            Some(property_name) => property_name,
            None => return,
        };
        let existing = self
            .backend
            .global_property_descriptor(&property_name)
            .cloned();
        let value = if descriptor.is_accessor() {
            Expression::Undefined
        } else {
            descriptor
                .value
                .as_ref()
                .map(|expression| self.materialize_static_expression(expression))
                .or_else(|| existing.as_ref().map(|state| state.value.clone()))
                .unwrap_or(Expression::Undefined)
        };
        let writable = if descriptor.is_accessor() {
            None
        } else {
            Some(
                descriptor
                    .writable
                    .or_else(|| existing.as_ref().and_then(|state| state.writable))
                    .unwrap_or(false),
            )
        };
        let enumerable = descriptor.enumerable.unwrap_or_else(|| {
            existing
                .as_ref()
                .map(|state| state.enumerable)
                .unwrap_or(false)
        });
        let configurable = descriptor.configurable.unwrap_or_else(|| {
            existing
                .as_ref()
                .map(|state| state.configurable)
                .unwrap_or(false)
        });
        self.backend.upsert_global_property_descriptor(
            property_name,
            GlobalPropertyDescriptorState {
                value,
                writable,
                enumerable,
                configurable,
            },
        );
    }

    fn define_object_property_from_descriptor(
        &mut self,
        name: &str,
        property: &Expression,
        descriptor: &PropertyDescriptorDefinition,
    ) {
        let property = self
            .resolve_property_key_expression(property)
            .unwrap_or_else(|| self.materialize_static_expression(property));
        let property_name = static_property_name_from_expression(&property);
        let existing_value = self
            .state
            .speculation
            .static_semantics
            .local_object_binding(name)
            .or_else(|| self.backend.global_object_binding(name))
            .and_then(|object_binding| object_binding_lookup_value(object_binding, &property))
            .cloned();
        let current_enumerable = property_name.as_ref().is_some_and(|property_name| {
            self.state
                .speculation
                .static_semantics
                .local_object_binding(name)
                .or_else(|| self.backend.global_object_binding(name))
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
                .map(|expression| self.materialize_static_expression(expression))
                .or_else(|| {
                    existing_value
                        .as_ref()
                        .map(|expression| self.materialize_static_expression(expression))
                })
                .unwrap_or(Expression::Undefined)
        };
        if let Some(object_binding) = self
            .state
            .speculation
            .static_semantics
            .local_object_binding_mut(name)
        {
            object_binding_define_property(
                object_binding,
                property.clone(),
                value.clone(),
                enumerable,
            );
        } else if self.binding_name_is_global(name) {
            self.backend
                .define_global_object_property(name, property, value, enumerable);
        }
    }

    fn define_prototype_object_property_from_descriptor(
        &mut self,
        name: &str,
        property: &Expression,
        descriptor: &PropertyDescriptorDefinition,
    ) {
        let property = self
            .resolve_property_key_expression(property)
            .unwrap_or_else(|| self.materialize_static_expression(property));
        let property_name = static_property_name_from_expression(&property);
        let existing_value = self
            .state
            .speculation
            .static_semantics
            .objects
            .local_prototype_object_bindings
            .get(name)
            .or_else(|| self.backend.global_prototype_object_binding(name))
            .and_then(|object_binding| object_binding_lookup_value(object_binding, &property))
            .cloned();
        let current_enumerable = property_name.as_ref().is_some_and(|property_name| {
            self.state
                .speculation
                .static_semantics
                .objects
                .local_prototype_object_bindings
                .get(name)
                .or_else(|| self.backend.global_prototype_object_binding(name))
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
                .map(|expression| self.materialize_static_expression(expression))
                .or_else(|| {
                    existing_value
                        .as_ref()
                        .map(|expression| self.materialize_static_expression(expression))
                })
                .unwrap_or(Expression::Undefined)
        };
        if let Some(object_binding) = self
            .state
            .speculation
            .static_semantics
            .objects
            .local_prototype_object_bindings
            .get_mut(name)
        {
            object_binding_define_property(
                object_binding,
                property.clone(),
                value.clone(),
                enumerable,
            );
        }
        if self.binding_name_is_global(name) {
            self.backend
                .define_global_prototype_object_property(name, property, value, enumerable);
        }
    }
}
