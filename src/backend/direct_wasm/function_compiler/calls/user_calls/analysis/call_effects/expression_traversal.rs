use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn collect_expression_call_effect_nonlocal_bindings(
        &self,
        expression: &Expression,
        current_function_name: Option<&str>,
        names: &mut HashSet<String>,
        visited: &mut HashSet<String>,
    ) {
        match expression {
            Expression::Call { callee, arguments }
            | Expression::SuperCall { callee, arguments }
            | Expression::New { callee, arguments } => {
                if let Some(LocalFunctionBinding::User(function_name)) = self
                    .resolve_function_binding_from_expression_with_context(
                        callee,
                        current_function_name,
                    )
                {
                    names.extend(
                        self.collect_user_function_call_effect_nonlocal_bindings_for_name(
                            &function_name,
                            visited,
                        ),
                    );
                }
                self.collect_expression_call_effect_nonlocal_bindings(
                    callee,
                    current_function_name,
                    names,
                    visited,
                );
                for argument in arguments {
                    match argument {
                        CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                            self.collect_expression_call_effect_nonlocal_bindings(
                                expression,
                                current_function_name,
                                names,
                                visited,
                            );
                        }
                    }
                }
            }
            Expression::AssignMember {
                object,
                property,
                value,
            } => {
                if let Some(LocalFunctionBinding::User(function_name)) =
                    self.resolve_member_setter_binding(object, property)
                {
                    names.extend(
                        self.collect_user_function_call_effect_nonlocal_bindings_for_name(
                            &function_name,
                            visited,
                        ),
                    );
                }
                self.collect_expression_call_effect_nonlocal_bindings(
                    object,
                    current_function_name,
                    names,
                    visited,
                );
                self.collect_expression_call_effect_nonlocal_bindings(
                    property,
                    current_function_name,
                    names,
                    visited,
                );
                self.collect_expression_call_effect_nonlocal_bindings(
                    value,
                    current_function_name,
                    names,
                    visited,
                );
            }
            Expression::AssignSuperMember { property, value } => {
                if let Some(effective_property) = self.resolve_property_key_expression(property) {
                    if let Some((_, binding)) = self
                        .resolve_super_runtime_prototype_binding_with_context(current_function_name)
                    {
                        if let Some(variants) =
                            self.resolve_user_super_setter_variants(&binding, &effective_property)
                        {
                            for (user_function, _) in variants {
                                names.extend(
                                    self.collect_user_function_call_effect_nonlocal_bindings_for_name(
                                        &user_function.name,
                                        visited,
                                    ),
                                );
                            }
                        }
                    } else if let Some(super_base) =
                        self.resolve_super_base_expression_with_context(current_function_name)
                        && let Some(LocalFunctionBinding::User(function_name)) =
                            self.resolve_member_setter_binding(&super_base, &effective_property)
                    {
                        names.extend(
                            self.collect_user_function_call_effect_nonlocal_bindings_for_name(
                                &function_name,
                                visited,
                            ),
                        );
                    }
                }
                self.collect_expression_call_effect_nonlocal_bindings(
                    property,
                    current_function_name,
                    names,
                    visited,
                );
                self.collect_expression_call_effect_nonlocal_bindings(
                    value,
                    current_function_name,
                    names,
                    visited,
                );
            }
            Expression::Member { object, property } => {
                self.collect_expression_call_effect_nonlocal_bindings(
                    object,
                    current_function_name,
                    names,
                    visited,
                );
                self.collect_expression_call_effect_nonlocal_bindings(
                    property,
                    current_function_name,
                    names,
                    visited,
                );
            }
            Expression::SuperMember { property } => {
                self.collect_expression_call_effect_nonlocal_bindings(
                    property,
                    current_function_name,
                    names,
                    visited,
                );
            }
            Expression::IteratorClose(value) => {
                let return_property = Expression::String("return".to_string());
                if let Some(LocalFunctionBinding::User(function_name)) = self
                    .resolve_member_function_binding(value, &return_property)
                    .or_else(|| {
                        let Expression::Identifier(iterator_name) = value.as_ref() else {
                            return None;
                        };
                        self.resolve_iterator_close_return_binding_in_function(
                            iterator_name,
                            current_function_name,
                        )
                    })
                {
                    names.extend(
                        self.collect_user_function_call_effect_nonlocal_bindings_for_name(
                            &function_name,
                            visited,
                        ),
                    );
                }
                self.collect_expression_call_effect_nonlocal_bindings(
                    value,
                    current_function_name,
                    names,
                    visited,
                );
            }
            Expression::Assign { value, .. }
            | Expression::Await(value)
            | Expression::EnumerateKeys(value)
            | Expression::GetIterator(value)
            | Expression::Unary {
                expression: value, ..
            } => self.collect_expression_call_effect_nonlocal_bindings(
                value,
                current_function_name,
                names,
                visited,
            ),
            Expression::Binary { left, right, .. } => {
                self.collect_expression_call_effect_nonlocal_bindings(
                    left,
                    current_function_name,
                    names,
                    visited,
                );
                self.collect_expression_call_effect_nonlocal_bindings(
                    right,
                    current_function_name,
                    names,
                    visited,
                );
            }
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                self.collect_expression_call_effect_nonlocal_bindings(
                    condition,
                    current_function_name,
                    names,
                    visited,
                );
                self.collect_expression_call_effect_nonlocal_bindings(
                    then_expression,
                    current_function_name,
                    names,
                    visited,
                );
                self.collect_expression_call_effect_nonlocal_bindings(
                    else_expression,
                    current_function_name,
                    names,
                    visited,
                );
            }
            Expression::Sequence(expressions) => {
                for expression in expressions {
                    self.collect_expression_call_effect_nonlocal_bindings(
                        expression,
                        current_function_name,
                        names,
                        visited,
                    );
                }
            }
            Expression::Array(elements) => {
                for element in elements {
                    match element {
                        ArrayElement::Expression(expression) | ArrayElement::Spread(expression) => {
                            self.collect_expression_call_effect_nonlocal_bindings(
                                expression,
                                current_function_name,
                                names,
                                visited,
                            );
                        }
                    }
                }
            }
            Expression::Object(entries) => {
                for entry in entries {
                    match entry {
                        ObjectEntry::Data { key, value } => {
                            self.collect_expression_call_effect_nonlocal_bindings(
                                key,
                                current_function_name,
                                names,
                                visited,
                            );
                            self.collect_expression_call_effect_nonlocal_bindings(
                                value,
                                current_function_name,
                                names,
                                visited,
                            );
                        }
                        ObjectEntry::Getter { key, .. } | ObjectEntry::Setter { key, .. } => {
                            self.collect_expression_call_effect_nonlocal_bindings(
                                key,
                                current_function_name,
                                names,
                                visited,
                            );
                        }
                        ObjectEntry::Spread(expression) => {
                            self.collect_expression_call_effect_nonlocal_bindings(
                                expression,
                                current_function_name,
                                names,
                                visited,
                            );
                        }
                    }
                }
            }
            Expression::Identifier(_)
            | Expression::Update { .. }
            | Expression::Number(_)
            | Expression::BigInt(_)
            | Expression::String(_)
            | Expression::Bool(_)
            | Expression::Null
            | Expression::Undefined
            | Expression::NewTarget
            | Expression::Sent
            | Expression::This => {}
        }
    }
}
