use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn collect_parameter_value_bindings_from_expression(
        &self,
        expression: &Expression,
        aliases: &mut HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<Expression>>>,
    ) {
        match expression {
            Expression::Call { callee, arguments } => {
                self.collect_parameter_value_bindings_from_expression(callee, aliases, bindings);
                self.register_parameter_value_bindings_for_call(
                    callee, arguments, aliases, bindings,
                );
                for argument in arguments {
                    let argument = match argument {
                        CallArgument::Expression(argument) | CallArgument::Spread(argument) => {
                            argument
                        }
                    };
                    self.collect_parameter_value_bindings_from_expression(
                        argument, aliases, bindings,
                    );
                }
            }
            Expression::Assign { name, value } => {
                self.collect_parameter_value_bindings_from_expression(value, aliases, bindings);
                aliases.insert(
                    name.clone(),
                    self.resolve_function_binding_from_expression_with_aliases(value, aliases),
                );
            }
            Expression::Member { object, property } => {
                self.collect_parameter_value_bindings_from_expression(object, aliases, bindings);
                self.collect_parameter_value_bindings_from_expression(property, aliases, bindings);
            }
            Expression::SuperMember { property } => {
                self.collect_parameter_value_bindings_from_expression(property, aliases, bindings);
            }
            Expression::AssignMember {
                object,
                property,
                value,
            } => {
                self.collect_parameter_value_bindings_from_expression(object, aliases, bindings);
                self.collect_parameter_value_bindings_from_expression(property, aliases, bindings);
                self.collect_parameter_value_bindings_from_expression(value, aliases, bindings);
            }
            Expression::AssignSuperMember { property, value } => {
                self.collect_parameter_value_bindings_from_expression(property, aliases, bindings);
                self.collect_parameter_value_bindings_from_expression(value, aliases, bindings);
            }
            Expression::Unary { expression, .. }
            | Expression::Await(expression)
            | Expression::EnumerateKeys(expression)
            | Expression::GetIterator(expression)
            | Expression::IteratorClose(expression) => {
                self.collect_parameter_value_bindings_from_expression(
                    expression, aliases, bindings,
                );
            }
            Expression::Array(elements) => {
                for element in elements {
                    let expression = match element {
                        ArrayElement::Expression(expression) | ArrayElement::Spread(expression) => {
                            expression
                        }
                    };
                    self.collect_parameter_value_bindings_from_expression(
                        expression, aliases, bindings,
                    );
                }
            }
            Expression::Object(entries) => {
                for entry in entries {
                    match entry {
                        ObjectEntry::Data { key, value } => {
                            self.collect_parameter_value_bindings_from_expression(
                                key, aliases, bindings,
                            );
                            self.collect_parameter_value_bindings_from_expression(
                                value, aliases, bindings,
                            );
                        }
                        ObjectEntry::Getter { key, getter } => {
                            self.collect_parameter_value_bindings_from_expression(
                                key, aliases, bindings,
                            );
                            self.collect_parameter_value_bindings_from_expression(
                                getter, aliases, bindings,
                            );
                        }
                        ObjectEntry::Setter { key, setter } => {
                            self.collect_parameter_value_bindings_from_expression(
                                key, aliases, bindings,
                            );
                            self.collect_parameter_value_bindings_from_expression(
                                setter, aliases, bindings,
                            );
                        }
                        ObjectEntry::Spread(expression) => {
                            self.collect_parameter_value_bindings_from_expression(
                                expression, aliases, bindings,
                            );
                        }
                    }
                }
            }
            Expression::Binary { left, right, .. } => {
                self.collect_parameter_value_bindings_from_expression(left, aliases, bindings);
                self.collect_parameter_value_bindings_from_expression(right, aliases, bindings);
            }
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                self.collect_parameter_value_bindings_from_expression(condition, aliases, bindings);
                self.collect_parameter_value_bindings_from_expression(
                    then_expression,
                    aliases,
                    bindings,
                );
                self.collect_parameter_value_bindings_from_expression(
                    else_expression,
                    aliases,
                    bindings,
                );
            }
            Expression::Sequence(expressions) => {
                for expression in expressions {
                    self.collect_parameter_value_bindings_from_expression(
                        expression, aliases, bindings,
                    );
                }
            }
            Expression::New { callee, arguments } | Expression::SuperCall { callee, arguments } => {
                self.collect_parameter_value_bindings_from_expression(callee, aliases, bindings);
                for argument in arguments {
                    let argument = match argument {
                        CallArgument::Expression(argument) | CallArgument::Spread(argument) => {
                            argument
                        }
                    };
                    self.collect_parameter_value_bindings_from_expression(
                        argument, aliases, bindings,
                    );
                }
            }
            Expression::Update { .. }
            | Expression::Number(_)
            | Expression::BigInt(_)
            | Expression::String(_)
            | Expression::Bool(_)
            | Expression::Null
            | Expression::Undefined
            | Expression::Identifier(_)
            | Expression::This
            | Expression::Sent
            | Expression::NewTarget => {}
        }
    }
}
