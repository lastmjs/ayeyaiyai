use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn materialize_global_expression(
        &self,
        expression: &Expression,
    ) -> Expression {
        match expression {
            Expression::Member { object, property } => {
                if let Some(array_binding) = self.infer_global_array_binding(object) {
                    if let Some(index) = argument_index_from_expression(property) {
                        if let Some(Some(value)) = array_binding.values.get(index as usize) {
                            return self.materialize_global_expression(value);
                        }
                        return Expression::Undefined;
                    }
                }
                if let Some(object_binding) = self.infer_global_object_binding(object) {
                    let materialized_property = self.materialize_global_expression(property);
                    if let Some(value) =
                        object_binding_lookup_value(&object_binding, &materialized_property)
                    {
                        return self.materialize_global_expression(value);
                    }
                    if static_property_name_from_expression(&materialized_property).is_some()
                        || object_binding_has_property(&object_binding, &materialized_property)
                    {
                        return Expression::Undefined;
                    }
                }
                if let Expression::String(text) = object.as_ref() {
                    if let Some(index) = argument_index_from_expression(property) {
                        return text
                            .chars()
                            .nth(index as usize)
                            .map(|character| Expression::String(character.to_string()))
                            .unwrap_or(Expression::Undefined);
                    }
                }
                Expression::Member {
                    object: Box::new(self.materialize_global_expression(object)),
                    property: Box::new(self.materialize_global_expression(property)),
                }
            }
            Expression::Unary { op, expression } => Expression::Unary {
                op: *op,
                expression: Box::new(self.materialize_global_expression(expression)),
            },
            Expression::Binary { op, left, right } => Expression::Binary {
                op: *op,
                left: Box::new(self.materialize_global_expression(left)),
                right: Box::new(self.materialize_global_expression(right)),
            },
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => Expression::Conditional {
                condition: Box::new(self.materialize_global_expression(condition)),
                then_expression: Box::new(self.materialize_global_expression(then_expression)),
                else_expression: Box::new(self.materialize_global_expression(else_expression)),
            },
            Expression::Sequence(expressions) => Expression::Sequence(
                expressions
                    .iter()
                    .map(|expression| self.materialize_global_expression(expression))
                    .collect(),
            ),
            Expression::Array(elements) => Expression::Array(
                elements
                    .iter()
                    .map(|element| match element {
                        crate::ir::hir::ArrayElement::Expression(expression) => {
                            crate::ir::hir::ArrayElement::Expression(
                                self.materialize_global_expression(expression),
                            )
                        }
                        crate::ir::hir::ArrayElement::Spread(expression) => {
                            crate::ir::hir::ArrayElement::Spread(
                                self.materialize_global_expression(expression),
                            )
                        }
                    })
                    .collect(),
            ),
            Expression::Object(entries) => Expression::Object(
                entries
                    .iter()
                    .map(|entry| match entry {
                        crate::ir::hir::ObjectEntry::Data { key, value } => {
                            crate::ir::hir::ObjectEntry::Data {
                                key: self.materialize_global_expression(key),
                                value: self.materialize_global_expression(value),
                            }
                        }
                        crate::ir::hir::ObjectEntry::Getter { key, getter } => {
                            crate::ir::hir::ObjectEntry::Getter {
                                key: self.materialize_global_expression(key),
                                getter: self.materialize_global_expression(getter),
                            }
                        }
                        crate::ir::hir::ObjectEntry::Setter { key, setter } => {
                            crate::ir::hir::ObjectEntry::Setter {
                                key: self.materialize_global_expression(key),
                                setter: self.materialize_global_expression(setter),
                            }
                        }
                        crate::ir::hir::ObjectEntry::Spread(expression) => {
                            crate::ir::hir::ObjectEntry::Spread(
                                self.materialize_global_expression(expression),
                            )
                        }
                    })
                    .collect(),
            ),
            Expression::Assign { name, value } => Expression::Assign {
                name: name.clone(),
                value: Box::new(self.materialize_global_expression(value)),
            },
            Expression::AssignMember {
                object,
                property,
                value,
            } => Expression::AssignMember {
                object: Box::new(self.materialize_global_expression(object)),
                property: Box::new(self.materialize_global_expression(property)),
                value: Box::new(self.materialize_global_expression(value)),
            },
            Expression::AssignSuperMember { property, value } => Expression::AssignSuperMember {
                property: Box::new(self.materialize_global_expression(property)),
                value: Box::new(self.materialize_global_expression(value)),
            },
            Expression::Await(value) => {
                Expression::Await(Box::new(self.materialize_global_expression(value)))
            }
            Expression::EnumerateKeys(value) => {
                Expression::EnumerateKeys(Box::new(self.materialize_global_expression(value)))
            }
            Expression::GetIterator(value) => {
                Expression::GetIterator(Box::new(self.materialize_global_expression(value)))
            }
            Expression::IteratorClose(value) => {
                Expression::IteratorClose(Box::new(self.materialize_global_expression(value)))
            }
            Expression::Call { callee, arguments } => {
                if let Some(value) = self.infer_static_call_result_expression(callee, arguments) {
                    return self.materialize_global_expression(&value);
                }
                Expression::Call {
                    callee: Box::new(self.materialize_global_expression(callee)),
                    arguments: arguments
                        .iter()
                        .map(|argument| match argument {
                            CallArgument::Expression(expression) => CallArgument::Expression(
                                self.materialize_global_expression(expression),
                            ),
                            CallArgument::Spread(expression) => {
                                CallArgument::Spread(self.materialize_global_expression(expression))
                            }
                        })
                        .collect(),
                }
            }
            Expression::New { callee, arguments } => Expression::New {
                callee: Box::new(self.materialize_global_expression(callee)),
                arguments: arguments
                    .iter()
                    .map(|argument| match argument {
                        CallArgument::Expression(expression) => {
                            CallArgument::Expression(self.materialize_global_expression(expression))
                        }
                        CallArgument::Spread(expression) => {
                            CallArgument::Spread(self.materialize_global_expression(expression))
                        }
                    })
                    .collect(),
            },
            _ => expression.clone(),
        }
    }

    pub(in crate::backend::direct_wasm) fn infer_static_call_result_expression(
        &self,
        callee: &Expression,
        arguments: &[CallArgument],
    ) -> Option<Expression> {
        let Expression::Identifier(_) = callee else {
            return None;
        };
        let user_function = match self.infer_global_function_binding(callee)? {
            LocalFunctionBinding::User(function_name) => {
                self.user_function_map.get(&function_name)?
            }
            LocalFunctionBinding::Builtin(_) => return None,
        };
        if user_function.is_async() {
            return None;
        }

        let summary = user_function.inline_summary.as_ref()?;
        if !summary.effects.is_empty() {
            return None;
        }
        let return_value = summary.return_value.as_ref()?;
        Some(self.substitute_global_user_function_argument_bindings(
            return_value,
            user_function,
            arguments,
        ))
    }

    pub(in crate::backend::direct_wasm) fn substitute_global_user_function_argument_bindings(
        &self,
        expression: &Expression,
        user_function: &UserFunction,
        arguments: &[CallArgument],
    ) -> Expression {
        let mut bindings = HashMap::new();
        for (index, param_name) in user_function.params.iter().enumerate() {
            let value = match arguments.get(index) {
                Some(CallArgument::Expression(expression))
                | Some(CallArgument::Spread(expression)) => expression.clone(),
                None => Expression::Undefined,
            };
            bindings.insert(param_name.clone(), value);
        }
        self.substitute_global_expression_bindings(expression, &bindings)
    }

    pub(in crate::backend::direct_wasm) fn substitute_global_expression_bindings(
        &self,
        expression: &Expression,
        bindings: &HashMap<String, Expression>,
    ) -> Expression {
        match expression {
            Expression::Identifier(name) => bindings
                .get(name)
                .cloned()
                .unwrap_or_else(|| expression.clone()),
            Expression::Member { object, property } => Expression::Member {
                object: Box::new(self.substitute_global_expression_bindings(object, bindings)),
                property: Box::new(self.substitute_global_expression_bindings(property, bindings)),
            },
            Expression::Assign { name, value } => Expression::Assign {
                name: name.clone(),
                value: Box::new(self.substitute_global_expression_bindings(value, bindings)),
            },
            Expression::AssignMember {
                object,
                property,
                value,
            } => Expression::AssignMember {
                object: Box::new(self.substitute_global_expression_bindings(object, bindings)),
                property: Box::new(self.substitute_global_expression_bindings(property, bindings)),
                value: Box::new(self.substitute_global_expression_bindings(value, bindings)),
            },
            Expression::Unary { op, expression } => Expression::Unary {
                op: *op,
                expression: Box::new(
                    self.substitute_global_expression_bindings(expression, bindings),
                ),
            },
            Expression::Binary { op, left, right } => Expression::Binary {
                op: *op,
                left: Box::new(self.substitute_global_expression_bindings(left, bindings)),
                right: Box::new(self.substitute_global_expression_bindings(right, bindings)),
            },
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => Expression::Conditional {
                condition: Box::new(
                    self.substitute_global_expression_bindings(condition, bindings),
                ),
                then_expression: Box::new(
                    self.substitute_global_expression_bindings(then_expression, bindings),
                ),
                else_expression: Box::new(
                    self.substitute_global_expression_bindings(else_expression, bindings),
                ),
            },
            Expression::Sequence(expressions) => Expression::Sequence(
                expressions
                    .iter()
                    .map(|expression| {
                        self.substitute_global_expression_bindings(expression, bindings)
                    })
                    .collect(),
            ),
            Expression::Array(elements) => Expression::Array(
                elements
                    .iter()
                    .map(|element| match element {
                        crate::ir::hir::ArrayElement::Expression(expression) => {
                            crate::ir::hir::ArrayElement::Expression(
                                self.substitute_global_expression_bindings(expression, bindings),
                            )
                        }
                        crate::ir::hir::ArrayElement::Spread(expression) => {
                            crate::ir::hir::ArrayElement::Spread(
                                self.substitute_global_expression_bindings(expression, bindings),
                            )
                        }
                    })
                    .collect(),
            ),
            Expression::Object(entries) => Expression::Object(
                entries
                    .iter()
                    .map(|entry| match entry {
                        crate::ir::hir::ObjectEntry::Data { key, value } => {
                            crate::ir::hir::ObjectEntry::Data {
                                key: self.substitute_global_expression_bindings(key, bindings),
                                value: self.substitute_global_expression_bindings(value, bindings),
                            }
                        }
                        crate::ir::hir::ObjectEntry::Getter { key, getter } => {
                            crate::ir::hir::ObjectEntry::Getter {
                                key: self.substitute_global_expression_bindings(key, bindings),
                                getter: self
                                    .substitute_global_expression_bindings(getter, bindings),
                            }
                        }
                        crate::ir::hir::ObjectEntry::Setter { key, setter } => {
                            crate::ir::hir::ObjectEntry::Setter {
                                key: self.substitute_global_expression_bindings(key, bindings),
                                setter: self
                                    .substitute_global_expression_bindings(setter, bindings),
                            }
                        }
                        crate::ir::hir::ObjectEntry::Spread(expression) => {
                            crate::ir::hir::ObjectEntry::Spread(
                                self.substitute_global_expression_bindings(expression, bindings),
                            )
                        }
                    })
                    .collect(),
            ),
            Expression::Call { callee, arguments } => Expression::Call {
                callee: Box::new(self.substitute_global_expression_bindings(callee, bindings)),
                arguments: arguments
                    .iter()
                    .map(|argument| match argument {
                        CallArgument::Expression(expression) => CallArgument::Expression(
                            self.substitute_global_expression_bindings(expression, bindings),
                        ),
                        CallArgument::Spread(expression) => CallArgument::Spread(
                            self.substitute_global_expression_bindings(expression, bindings),
                        ),
                    })
                    .collect(),
            },
            Expression::New { callee, arguments } => Expression::New {
                callee: Box::new(self.substitute_global_expression_bindings(callee, bindings)),
                arguments: arguments
                    .iter()
                    .map(|argument| match argument {
                        CallArgument::Expression(expression) => CallArgument::Expression(
                            self.substitute_global_expression_bindings(expression, bindings),
                        ),
                        CallArgument::Spread(expression) => CallArgument::Spread(
                            self.substitute_global_expression_bindings(expression, bindings),
                        ),
                    })
                    .collect(),
            },
            _ => expression.clone(),
        }
    }
}
