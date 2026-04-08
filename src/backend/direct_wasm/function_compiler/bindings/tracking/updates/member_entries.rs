use super::*;

impl<'a> FunctionCompiler<'a> {
    fn set_member_function_binding_entry(
        &mut self,
        key: &MemberFunctionBindingKey,
        binding: LocalFunctionBinding,
    ) {
        self.state
            .speculation
            .static_semantics
            .objects
            .member_function_bindings
            .insert(key.clone(), binding.clone());
        if self.binding_key_is_global(key) {
            self.backend
                .set_global_member_function_binding(key.clone(), binding);
        }
    }

    fn clear_member_function_binding_entry(&mut self, key: &MemberFunctionBindingKey) {
        self.state
            .speculation
            .static_semantics
            .objects
            .member_function_bindings
            .remove(key);
        if self.binding_key_is_global(key) {
            self.backend.clear_global_member_function_binding(key);
        }
    }

    fn set_member_getter_binding_entry(
        &mut self,
        key: &MemberFunctionBindingKey,
        binding: LocalFunctionBinding,
    ) {
        self.state
            .speculation
            .static_semantics
            .objects
            .member_getter_bindings
            .insert(key.clone(), binding.clone());
        if self.binding_key_is_global(key) {
            self.backend
                .set_global_member_getter_binding(key.clone(), binding);
        }
    }

    fn clear_member_getter_binding_entry(&mut self, key: &MemberFunctionBindingKey) {
        self.state
            .speculation
            .static_semantics
            .objects
            .member_getter_bindings
            .remove(key);
        if self.binding_key_is_global(key) {
            self.backend.clear_global_member_getter_binding(key);
        }
    }

    fn set_member_setter_binding_entry(
        &mut self,
        key: &MemberFunctionBindingKey,
        binding: LocalFunctionBinding,
    ) {
        self.state
            .speculation
            .static_semantics
            .objects
            .member_setter_bindings
            .insert(key.clone(), binding.clone());
        if self.binding_key_is_global(key) {
            self.backend
                .set_global_member_setter_binding(key.clone(), binding);
        }
    }

    fn clear_member_setter_binding_entry(&mut self, key: &MemberFunctionBindingKey) {
        self.state
            .speculation
            .static_semantics
            .objects
            .member_setter_bindings
            .remove(key);
        if self.binding_key_is_global(key) {
            self.backend.clear_global_member_setter_binding(key);
        }
    }

    pub(in crate::backend::direct_wasm) fn update_member_function_binding_from_expression(
        &mut self,
        expression: &Expression,
    ) {
        match expression {
            Expression::Member { object, property } => {
                self.update_member_function_binding_from_expression(object);
                self.update_member_function_binding_from_expression(property);
            }
            Expression::SuperMember { property } => {
                self.update_member_function_binding_from_expression(property);
            }
            Expression::Assign { value, .. }
            | Expression::Await(value)
            | Expression::EnumerateKeys(value)
            | Expression::GetIterator(value)
            | Expression::IteratorClose(value)
            | Expression::Unary {
                expression: value, ..
            } => self.update_member_function_binding_from_expression(value),
            Expression::AssignMember {
                object,
                property,
                value,
            } => {
                self.update_member_function_binding_from_expression(object);
                self.update_member_function_binding_from_expression(property);
                self.update_member_function_binding_from_expression(value);
            }
            Expression::AssignSuperMember { property, value } => {
                self.update_member_function_binding_from_expression(property);
                self.update_member_function_binding_from_expression(value);
            }
            Expression::Binary { left, right, .. } => {
                self.update_member_function_binding_from_expression(left);
                self.update_member_function_binding_from_expression(right);
            }
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                self.update_member_function_binding_from_expression(condition);
                self.update_member_function_binding_from_expression(then_expression);
                self.update_member_function_binding_from_expression(else_expression);
            }
            Expression::Sequence(expressions) => {
                for expression in expressions {
                    self.update_member_function_binding_from_expression(expression);
                }
            }
            Expression::Call { callee, arguments }
            | Expression::SuperCall { callee, arguments }
            | Expression::New { callee, arguments } => {
                self.update_member_function_binding_from_expression(callee);
                for argument in arguments {
                    match argument {
                        CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                            self.update_member_function_binding_from_expression(expression);
                        }
                    }
                }
            }
            Expression::Array(elements) => {
                for element in elements {
                    match element {
                        crate::ir::hir::ArrayElement::Expression(expression)
                        | crate::ir::hir::ArrayElement::Spread(expression) => {
                            self.update_member_function_binding_from_expression(expression);
                        }
                    }
                }
            }
            Expression::Object(entries) => {
                for entry in entries {
                    match entry {
                        crate::ir::hir::ObjectEntry::Data { key, value } => {
                            self.update_member_function_binding_from_expression(key);
                            self.update_member_function_binding_from_expression(value);
                        }
                        crate::ir::hir::ObjectEntry::Getter { key, getter } => {
                            self.update_member_function_binding_from_expression(key);
                            self.update_member_function_binding_from_expression(getter);
                        }
                        crate::ir::hir::ObjectEntry::Setter { key, setter } => {
                            self.update_member_function_binding_from_expression(key);
                            self.update_member_function_binding_from_expression(setter);
                        }
                        crate::ir::hir::ObjectEntry::Spread(expression) => {
                            self.update_member_function_binding_from_expression(expression);
                        }
                    }
                }
            }
            _ => {}
        }
        let Expression::Call { callee, arguments } = expression else {
            return;
        };
        let Expression::Member { object, property } = callee.as_ref() else {
            return;
        };
        if !matches!(object.as_ref(), Expression::Identifier(name) if name == "Object") {
            return;
        }
        if !matches!(property.as_ref(), Expression::String(name) if name == "defineProperty") {
            return;
        }

        let [
            CallArgument::Expression(target),
            CallArgument::Expression(property),
            CallArgument::Expression(descriptor),
            ..,
        ] = arguments.as_slice()
        else {
            return;
        };

        let Some(key) = self.member_function_binding_key(target, property) else {
            return;
        };
        let has_value_field = self.descriptor_expression_has_named_field(descriptor, "value");
        let has_get_field = self.descriptor_expression_has_named_field(descriptor, "get");
        let has_set_field = self.descriptor_expression_has_named_field(descriptor, "set");
        let value_binding = self.resolve_function_binding_from_descriptor_expression(descriptor);
        let getter_binding = self.resolve_getter_binding_from_descriptor_expression(descriptor);
        let setter_binding = self.resolve_setter_binding_from_descriptor_expression(descriptor);

        if let Some(binding) = value_binding {
            self.set_member_function_binding_entry(&key, binding);
        } else if has_value_field {
            self.clear_member_function_binding_entry(&key);
        }

        if let Some(binding) = getter_binding {
            self.set_member_getter_binding_entry(&key, binding);
        } else if has_get_field {
            self.clear_member_getter_binding_entry(&key);
        }

        if let Some(binding) = setter_binding {
            self.set_member_setter_binding_entry(&key, binding);
        } else if has_set_field {
            self.clear_member_setter_binding_entry(&key);
        }
    }

    pub(in crate::backend::direct_wasm) fn update_member_function_assignment_binding(
        &mut self,
        object: &Expression,
        property: &Expression,
        value: &Expression,
    ) {
        let Some(key) = self.member_function_binding_key(object, property) else {
            return;
        };
        let value_binding = self.resolve_function_binding_from_expression(value);

        if let Some(binding) = value_binding {
            self.set_member_function_binding_entry(&key, binding);
        } else {
            self.clear_member_function_binding_entry(&key);
        }

        self.clear_member_getter_binding_entry(&key);
        self.clear_member_setter_binding_entry(&key);
    }

    pub(in crate::backend::direct_wasm) fn update_local_function_binding(
        &mut self,
        name: &str,
        value: &Expression,
    ) {
        let Some(function_name) = self.resolve_function_binding_from_expression(value) else {
            self.state
                .speculation
                .static_semantics
                .clear_local_function_binding(name);
            return;
        };
        self.state
            .speculation
            .static_semantics
            .set_local_function_binding(name, function_name);
    }
}
