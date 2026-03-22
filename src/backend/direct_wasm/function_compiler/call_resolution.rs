use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_user_function_from_callee_name(
        &self,
        name: &str,
    ) -> Option<&UserFunction> {
        let resolved_name = self
            .resolve_current_local_binding(name)
            .map(|(resolved_name, _)| resolved_name)
            .unwrap_or_else(|| name.to_string());
        if let Some(LocalFunctionBinding::User(function_name)) =
            self.local_function_bindings.get(&resolved_name)
        {
            return self.module.user_function_map.get(function_name);
        }
        if let Some(LocalFunctionBinding::User(function_name)) =
            self.module.global_function_bindings.get(name)
        {
            return self.module.user_function_map.get(function_name);
        }
        if is_internal_user_function_identifier(name) {
            return self.module.user_function_map.get(name);
        }
        None
    }

    pub(in crate::backend::direct_wasm) fn resolve_returned_member_value_from_expression(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> Option<Expression> {
        let Expression::String(property_name) = property else {
            return None;
        };

        let (callee, arguments) = match object {
            Expression::Call { callee, arguments } | Expression::New { callee, arguments } => {
                (callee.as_ref(), arguments.as_slice())
            }
            _ => return None,
        };

        let Expression::Identifier(callee_name) = callee else {
            return None;
        };
        let user_function = self.resolve_user_function_from_callee_name(callee_name)?;
        let binding = user_function
            .returned_member_value_bindings
            .iter()
            .find(|binding| binding.property == *property_name)?;

        Some(self.substitute_user_function_argument_bindings(
            &binding.value,
            user_function,
            arguments,
        ))
    }

    pub(in crate::backend::direct_wasm) fn resolve_returned_object_binding_from_call(
        &self,
        callee: &Expression,
        arguments: &[CallArgument],
    ) -> Option<ObjectValueBinding> {
        let Expression::Identifier(callee_name) = callee else {
            return None;
        };
        let user_function = self.resolve_user_function_from_callee_name(callee_name)?;
        if user_function.returned_member_value_bindings.is_empty() {
            return None;
        }
        let mut object_binding = empty_object_value_binding();
        for binding in &user_function.returned_member_value_bindings {
            let value = self.substitute_user_function_argument_bindings(
                &binding.value,
                user_function,
                arguments,
            );
            object_binding_set_property(
                &mut object_binding,
                Expression::String(binding.property.clone()),
                value,
            );
        }
        Some(object_binding)
    }

    pub(in crate::backend::direct_wasm) fn substitute_user_function_argument_bindings(
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
        self.substitute_expression_bindings(expression, &bindings)
    }

    pub(in crate::backend::direct_wasm) fn substitute_expression_bindings(
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
                object: Box::new(self.substitute_expression_bindings(object, bindings)),
                property: Box::new(self.substitute_expression_bindings(property, bindings)),
            },
            Expression::Assign { name, value } => Expression::Assign {
                name: name.clone(),
                value: Box::new(self.substitute_expression_bindings(value, bindings)),
            },
            Expression::AssignMember {
                object,
                property,
                value,
            } => Expression::AssignMember {
                object: Box::new(self.substitute_expression_bindings(object, bindings)),
                property: Box::new(self.substitute_expression_bindings(property, bindings)),
                value: Box::new(self.substitute_expression_bindings(value, bindings)),
            },
            Expression::AssignSuperMember { property, value } => Expression::AssignSuperMember {
                property: Box::new(self.substitute_expression_bindings(property, bindings)),
                value: Box::new(self.substitute_expression_bindings(value, bindings)),
            },
            Expression::Await(value) => Expression::Await(Box::new(
                self.substitute_expression_bindings(value, bindings),
            )),
            Expression::EnumerateKeys(value) => Expression::EnumerateKeys(Box::new(
                self.substitute_expression_bindings(value, bindings),
            )),
            Expression::GetIterator(value) => Expression::GetIterator(Box::new(
                self.substitute_expression_bindings(value, bindings),
            )),
            Expression::IteratorClose(value) => Expression::IteratorClose(Box::new(
                self.substitute_expression_bindings(value, bindings),
            )),
            Expression::Unary { op, expression } => Expression::Unary {
                op: *op,
                expression: Box::new(self.substitute_expression_bindings(expression, bindings)),
            },
            Expression::Binary { op, left, right } => Expression::Binary {
                op: *op,
                left: Box::new(self.substitute_expression_bindings(left, bindings)),
                right: Box::new(self.substitute_expression_bindings(right, bindings)),
            },
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => Expression::Conditional {
                condition: Box::new(self.substitute_expression_bindings(condition, bindings)),
                then_expression: Box::new(
                    self.substitute_expression_bindings(then_expression, bindings),
                ),
                else_expression: Box::new(
                    self.substitute_expression_bindings(else_expression, bindings),
                ),
            },
            Expression::Sequence(expressions) => Expression::Sequence(
                expressions
                    .iter()
                    .map(|expression| self.substitute_expression_bindings(expression, bindings))
                    .collect(),
            ),
            Expression::Call { callee, arguments } => Expression::Call {
                callee: Box::new(self.substitute_expression_bindings(callee, bindings)),
                arguments: arguments
                    .iter()
                    .map(|argument| match argument {
                        CallArgument::Expression(expression) => CallArgument::Expression(
                            self.substitute_expression_bindings(expression, bindings),
                        ),
                        CallArgument::Spread(expression) => CallArgument::Spread(
                            self.substitute_expression_bindings(expression, bindings),
                        ),
                    })
                    .collect(),
            },
            Expression::SuperCall { callee, arguments } => Expression::SuperCall {
                callee: Box::new(self.substitute_expression_bindings(callee, bindings)),
                arguments: arguments
                    .iter()
                    .map(|argument| match argument {
                        CallArgument::Expression(expression) => CallArgument::Expression(
                            self.substitute_expression_bindings(expression, bindings),
                        ),
                        CallArgument::Spread(expression) => CallArgument::Spread(
                            self.substitute_expression_bindings(expression, bindings),
                        ),
                    })
                    .collect(),
            },
            Expression::New { callee, arguments } => Expression::New {
                callee: Box::new(self.substitute_expression_bindings(callee, bindings)),
                arguments: arguments
                    .iter()
                    .map(|argument| match argument {
                        CallArgument::Expression(expression) => CallArgument::Expression(
                            self.substitute_expression_bindings(expression, bindings),
                        ),
                        CallArgument::Spread(expression) => CallArgument::Spread(
                            self.substitute_expression_bindings(expression, bindings),
                        ),
                    })
                    .collect(),
            },
            Expression::Array(elements) => Expression::Array(
                elements
                    .iter()
                    .map(|element| match element {
                        crate::ir::hir::ArrayElement::Expression(expression) => {
                            crate::ir::hir::ArrayElement::Expression(
                                self.substitute_expression_bindings(expression, bindings),
                            )
                        }
                        crate::ir::hir::ArrayElement::Spread(expression) => {
                            crate::ir::hir::ArrayElement::Spread(
                                self.substitute_expression_bindings(expression, bindings),
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
                                key: self.substitute_expression_bindings(key, bindings),
                                value: self.substitute_expression_bindings(value, bindings),
                            }
                        }
                        crate::ir::hir::ObjectEntry::Getter { key, getter } => {
                            crate::ir::hir::ObjectEntry::Getter {
                                key: self.substitute_expression_bindings(key, bindings),
                                getter: self.substitute_expression_bindings(getter, bindings),
                            }
                        }
                        crate::ir::hir::ObjectEntry::Setter { key, setter } => {
                            crate::ir::hir::ObjectEntry::Setter {
                                key: self.substitute_expression_bindings(key, bindings),
                                setter: self.substitute_expression_bindings(setter, bindings),
                            }
                        }
                        crate::ir::hir::ObjectEntry::Spread(expression) => {
                            crate::ir::hir::ObjectEntry::Spread(
                                self.substitute_expression_bindings(expression, bindings),
                            )
                        }
                    })
                    .collect(),
            ),
            Expression::SuperMember { property } => Expression::SuperMember {
                property: Box::new(self.substitute_expression_bindings(property, bindings)),
            },
            _ => expression.clone(),
        }
    }

    pub(in crate::backend::direct_wasm) fn substitute_user_function_call_frame_bindings(
        &self,
        expression: &Expression,
        user_function: &UserFunction,
        arguments: &[CallArgument],
        this_binding: &Expression,
        arguments_binding: &Expression,
    ) -> Expression {
        let substituted =
            self.substitute_user_function_argument_bindings(expression, user_function, arguments);
        self.substitute_call_frame_special_bindings(
            &substituted,
            user_function,
            this_binding,
            arguments_binding,
        )
    }

    pub(in crate::backend::direct_wasm) fn substitute_call_frame_special_bindings(
        &self,
        expression: &Expression,
        user_function: &UserFunction,
        this_binding: &Expression,
        arguments_binding: &Expression,
    ) -> Expression {
        let arguments_shadowed = user_function.body_declares_arguments_binding
            || user_function
                .params
                .iter()
                .any(|param| param == "arguments");
        match expression {
            Expression::This if !user_function.lexical_this => this_binding.clone(),
            Expression::Identifier(name) if name == "arguments" && !arguments_shadowed => {
                arguments_binding.clone()
            }
            Expression::Member { object, property } => Expression::Member {
                object: Box::new(self.substitute_call_frame_special_bindings(
                    object,
                    user_function,
                    this_binding,
                    arguments_binding,
                )),
                property: Box::new(self.substitute_call_frame_special_bindings(
                    property,
                    user_function,
                    this_binding,
                    arguments_binding,
                )),
            },
            Expression::Assign { name, value } => Expression::Assign {
                name: name.clone(),
                value: Box::new(self.substitute_call_frame_special_bindings(
                    value,
                    user_function,
                    this_binding,
                    arguments_binding,
                )),
            },
            Expression::AssignMember {
                object,
                property,
                value,
            } => Expression::AssignMember {
                object: Box::new(self.substitute_call_frame_special_bindings(
                    object,
                    user_function,
                    this_binding,
                    arguments_binding,
                )),
                property: Box::new(self.substitute_call_frame_special_bindings(
                    property,
                    user_function,
                    this_binding,
                    arguments_binding,
                )),
                value: Box::new(self.substitute_call_frame_special_bindings(
                    value,
                    user_function,
                    this_binding,
                    arguments_binding,
                )),
            },
            Expression::AssignSuperMember { property, value } => Expression::AssignSuperMember {
                property: Box::new(self.substitute_call_frame_special_bindings(
                    property,
                    user_function,
                    this_binding,
                    arguments_binding,
                )),
                value: Box::new(self.substitute_call_frame_special_bindings(
                    value,
                    user_function,
                    this_binding,
                    arguments_binding,
                )),
            },
            Expression::Await(value) => {
                Expression::Await(Box::new(self.substitute_call_frame_special_bindings(
                    value,
                    user_function,
                    this_binding,
                    arguments_binding,
                )))
            }
            Expression::EnumerateKeys(value) => {
                Expression::EnumerateKeys(Box::new(self.substitute_call_frame_special_bindings(
                    value,
                    user_function,
                    this_binding,
                    arguments_binding,
                )))
            }
            Expression::GetIterator(value) => {
                Expression::GetIterator(Box::new(self.substitute_call_frame_special_bindings(
                    value,
                    user_function,
                    this_binding,
                    arguments_binding,
                )))
            }
            Expression::IteratorClose(value) => {
                Expression::IteratorClose(Box::new(self.substitute_call_frame_special_bindings(
                    value,
                    user_function,
                    this_binding,
                    arguments_binding,
                )))
            }
            Expression::Unary { op, expression } => Expression::Unary {
                op: *op,
                expression: Box::new(self.substitute_call_frame_special_bindings(
                    expression,
                    user_function,
                    this_binding,
                    arguments_binding,
                )),
            },
            Expression::Binary { op, left, right } => Expression::Binary {
                op: *op,
                left: Box::new(self.substitute_call_frame_special_bindings(
                    left,
                    user_function,
                    this_binding,
                    arguments_binding,
                )),
                right: Box::new(self.substitute_call_frame_special_bindings(
                    right,
                    user_function,
                    this_binding,
                    arguments_binding,
                )),
            },
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => Expression::Conditional {
                condition: Box::new(self.substitute_call_frame_special_bindings(
                    condition,
                    user_function,
                    this_binding,
                    arguments_binding,
                )),
                then_expression: Box::new(self.substitute_call_frame_special_bindings(
                    then_expression,
                    user_function,
                    this_binding,
                    arguments_binding,
                )),
                else_expression: Box::new(self.substitute_call_frame_special_bindings(
                    else_expression,
                    user_function,
                    this_binding,
                    arguments_binding,
                )),
            },
            Expression::Sequence(expressions) => Expression::Sequence(
                expressions
                    .iter()
                    .map(|expression| {
                        self.substitute_call_frame_special_bindings(
                            expression,
                            user_function,
                            this_binding,
                            arguments_binding,
                        )
                    })
                    .collect(),
            ),
            Expression::Call { callee, arguments } => Expression::Call {
                callee: Box::new(self.substitute_call_frame_special_bindings(
                    callee,
                    user_function,
                    this_binding,
                    arguments_binding,
                )),
                arguments: arguments
                    .iter()
                    .map(|argument| match argument {
                        CallArgument::Expression(expression) => {
                            CallArgument::Expression(self.substitute_call_frame_special_bindings(
                                expression,
                                user_function,
                                this_binding,
                                arguments_binding,
                            ))
                        }
                        CallArgument::Spread(expression) => {
                            CallArgument::Spread(self.substitute_call_frame_special_bindings(
                                expression,
                                user_function,
                                this_binding,
                                arguments_binding,
                            ))
                        }
                    })
                    .collect(),
            },
            Expression::SuperCall { callee, arguments } => Expression::SuperCall {
                callee: Box::new(self.substitute_call_frame_special_bindings(
                    callee,
                    user_function,
                    this_binding,
                    arguments_binding,
                )),
                arguments: arguments
                    .iter()
                    .map(|argument| match argument {
                        CallArgument::Expression(expression) => {
                            CallArgument::Expression(self.substitute_call_frame_special_bindings(
                                expression,
                                user_function,
                                this_binding,
                                arguments_binding,
                            ))
                        }
                        CallArgument::Spread(expression) => {
                            CallArgument::Spread(self.substitute_call_frame_special_bindings(
                                expression,
                                user_function,
                                this_binding,
                                arguments_binding,
                            ))
                        }
                    })
                    .collect(),
            },
            Expression::New { callee, arguments } => Expression::New {
                callee: Box::new(self.substitute_call_frame_special_bindings(
                    callee,
                    user_function,
                    this_binding,
                    arguments_binding,
                )),
                arguments: arguments
                    .iter()
                    .map(|argument| match argument {
                        CallArgument::Expression(expression) => {
                            CallArgument::Expression(self.substitute_call_frame_special_bindings(
                                expression,
                                user_function,
                                this_binding,
                                arguments_binding,
                            ))
                        }
                        CallArgument::Spread(expression) => {
                            CallArgument::Spread(self.substitute_call_frame_special_bindings(
                                expression,
                                user_function,
                                this_binding,
                                arguments_binding,
                            ))
                        }
                    })
                    .collect(),
            },
            Expression::Array(elements) => Expression::Array(
                elements
                    .iter()
                    .map(|element| match element {
                        crate::ir::hir::ArrayElement::Expression(expression) => {
                            crate::ir::hir::ArrayElement::Expression(
                                self.substitute_call_frame_special_bindings(
                                    expression,
                                    user_function,
                                    this_binding,
                                    arguments_binding,
                                ),
                            )
                        }
                        crate::ir::hir::ArrayElement::Spread(expression) => {
                            crate::ir::hir::ArrayElement::Spread(
                                self.substitute_call_frame_special_bindings(
                                    expression,
                                    user_function,
                                    this_binding,
                                    arguments_binding,
                                ),
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
                                key: self.substitute_call_frame_special_bindings(
                                    key,
                                    user_function,
                                    this_binding,
                                    arguments_binding,
                                ),
                                value: self.substitute_call_frame_special_bindings(
                                    value,
                                    user_function,
                                    this_binding,
                                    arguments_binding,
                                ),
                            }
                        }
                        crate::ir::hir::ObjectEntry::Getter { key, getter } => {
                            crate::ir::hir::ObjectEntry::Getter {
                                key: self.substitute_call_frame_special_bindings(
                                    key,
                                    user_function,
                                    this_binding,
                                    arguments_binding,
                                ),
                                getter: self.substitute_call_frame_special_bindings(
                                    getter,
                                    user_function,
                                    this_binding,
                                    arguments_binding,
                                ),
                            }
                        }
                        crate::ir::hir::ObjectEntry::Setter { key, setter } => {
                            crate::ir::hir::ObjectEntry::Setter {
                                key: self.substitute_call_frame_special_bindings(
                                    key,
                                    user_function,
                                    this_binding,
                                    arguments_binding,
                                ),
                                setter: self.substitute_call_frame_special_bindings(
                                    setter,
                                    user_function,
                                    this_binding,
                                    arguments_binding,
                                ),
                            }
                        }
                        crate::ir::hir::ObjectEntry::Spread(expression) => {
                            crate::ir::hir::ObjectEntry::Spread(
                                self.substitute_call_frame_special_bindings(
                                    expression,
                                    user_function,
                                    this_binding,
                                    arguments_binding,
                                ),
                            )
                        }
                    })
                    .collect(),
            ),
            Expression::SuperMember { property } => Expression::SuperMember {
                property: Box::new(self.substitute_call_frame_special_bindings(
                    property,
                    user_function,
                    this_binding,
                    arguments_binding,
                )),
            },
            _ => expression.clone(),
        }
    }

    pub(in crate::backend::direct_wasm) fn emit_inline_user_function_summary_with_explicit_call_frame(
        &mut self,
        user_function: &UserFunction,
        arguments: &[Expression],
        this_binding: &Expression,
        result_local: u32,
    ) -> DirectResult<bool> {
        let call_arguments = arguments
            .iter()
            .cloned()
            .map(CallArgument::Expression)
            .collect::<Vec<_>>();
        let arguments_binding = Expression::Array(
            arguments
                .iter()
                .cloned()
                .map(crate::ir::hir::ArrayElement::Expression)
                .collect(),
        );

        if let Some(summary) = user_function.inline_summary.as_ref() {
            let previous_strict_mode = self.strict_mode;
            let previous_user_function_name = self.current_user_function_name.clone();
            self.strict_mode = user_function.strict;
            self.current_user_function_name = Some(user_function.name.clone());
            for effect in &summary.effects {
                match effect {
                    InlineFunctionEffect::Assign { name, value } => {
                        self.emit_statement(&Statement::Assign {
                            name: name.clone(),
                            value: self.substitute_user_function_call_frame_bindings(
                                value,
                                user_function,
                                &call_arguments,
                                this_binding,
                                &arguments_binding,
                            ),
                        })?;
                    }
                    InlineFunctionEffect::Update { name, op, prefix } => {
                        self.emit_numeric_expression(&Expression::Update {
                            name: name.clone(),
                            op: *op,
                            prefix: *prefix,
                        })?;
                        self.instructions.push(0x1a);
                    }
                    InlineFunctionEffect::Expression(expression) => {
                        let substituted = self.substitute_user_function_call_frame_bindings(
                            expression,
                            user_function,
                            &call_arguments,
                            this_binding,
                            &arguments_binding,
                        );
                        self.emit_numeric_expression(&substituted)?;
                        self.instructions.push(0x1a);
                    }
                }
            }
            if let Some(return_value) = summary.return_value.as_ref() {
                let substituted = self.substitute_user_function_call_frame_bindings(
                    return_value,
                    user_function,
                    &call_arguments,
                    this_binding,
                    &arguments_binding,
                );
                self.emit_numeric_expression(&substituted)?;
                self.push_local_set(result_local);
            } else {
                self.push_i32_const(JS_UNDEFINED_TAG);
                self.push_local_set(result_local);
            }
            self.strict_mode = previous_strict_mode;
            self.current_user_function_name = previous_user_function_name;
            return Ok(true);
        }

        let Some(function) = self
            .resolve_registered_function_declaration(&user_function.name)
            .cloned()
        else {
            return Ok(false);
        };
        let previous_strict_mode = self.strict_mode;
        let previous_user_function_name = self.current_user_function_name.clone();
        self.strict_mode = user_function.strict;
        self.current_user_function_name = Some(user_function.name.clone());
        let Some((terminal_statement, effect_statements)) = function.body.split_last() else {
            self.strict_mode = previous_strict_mode;
            self.current_user_function_name = previous_user_function_name;
            return Ok(false);
        };
        for statement in effect_statements {
            match statement {
                Statement::Assign { name, value } => {
                    self.emit_statement(&Statement::Assign {
                        name: name.clone(),
                        value: self.substitute_user_function_call_frame_bindings(
                            value,
                            user_function,
                            &call_arguments,
                            this_binding,
                            &arguments_binding,
                        ),
                    })?;
                }
                Statement::Expression(Expression::Update { name, op, prefix }) => {
                    self.emit_numeric_expression(&Expression::Update {
                        name: name.clone(),
                        op: *op,
                        prefix: *prefix,
                    })?;
                    self.instructions.push(0x1a);
                }
                Statement::Expression(expression) => {
                    let substituted = self.substitute_user_function_call_frame_bindings(
                        expression,
                        user_function,
                        &call_arguments,
                        this_binding,
                        &arguments_binding,
                    );
                    self.emit_numeric_expression(&substituted)?;
                    self.instructions.push(0x1a);
                }
                Statement::Block { body } if body.is_empty() => {}
                _ => {
                    self.strict_mode = previous_strict_mode;
                    self.current_user_function_name = previous_user_function_name;
                    return Ok(false);
                }
            }
        }
        match terminal_statement {
            Statement::Return(return_value) => {
                let substituted = self.substitute_user_function_call_frame_bindings(
                    return_value,
                    user_function,
                    &call_arguments,
                    this_binding,
                    &arguments_binding,
                );
                self.emit_numeric_expression(&substituted)?;
                self.push_local_set(result_local);
            }
            Statement::Throw(throw_value) => {
                let substituted = self.substitute_user_function_call_frame_bindings(
                    throw_value,
                    user_function,
                    &call_arguments,
                    this_binding,
                    &arguments_binding,
                );
                self.emit_statement(&Statement::Throw(substituted))?;
            }
            _ => {
                self.strict_mode = previous_strict_mode;
                self.current_user_function_name = previous_user_function_name;
                return Ok(false);
            }
        }
        self.strict_mode = previous_strict_mode;
        self.current_user_function_name = previous_user_function_name;
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn resolve_function_binding_static_return_expression_with_call_frame(
        &self,
        binding: &LocalFunctionBinding,
        arguments: &[Expression],
        this_binding: &Expression,
    ) -> Option<Expression> {
        let LocalFunctionBinding::User(function_name) = binding else {
            return None;
        };
        let user_function = self.module.user_function_map.get(function_name)?;
        let summary = user_function.inline_summary.as_ref()?;
        let return_value = summary.return_value.as_ref()?;
        let call_arguments = arguments
            .iter()
            .cloned()
            .map(CallArgument::Expression)
            .collect::<Vec<_>>();
        let arguments_binding = Expression::Array(
            arguments
                .iter()
                .cloned()
                .map(crate::ir::hir::ArrayElement::Expression)
                .collect(),
        );
        Some(self.substitute_user_function_call_frame_bindings(
            return_value,
            user_function,
            &call_arguments,
            this_binding,
            &arguments_binding,
        ))
    }

    pub(in crate::backend::direct_wasm) fn resolve_inline_call_from_returned_member(
        &self,
        object: &Expression,
        property: &Expression,
        arguments: &[CallArgument],
    ) -> Option<Expression> {
        let Expression::String(property_name) = property else {
            return None;
        };

        let (outer_callee, outer_arguments) = match object {
            Expression::Call { callee, arguments } | Expression::New { callee, arguments } => {
                (callee.as_ref(), arguments.as_slice())
            }
            _ => return None,
        };

        let Expression::Identifier(outer_name) = outer_callee else {
            return None;
        };
        let outer_user_function = self.resolve_user_function_from_callee_name(outer_name)?;
        let returned_value = outer_user_function
            .returned_member_value_bindings
            .iter()
            .find(|binding| binding.property == *property_name)?
            .value
            .clone();
        let substituted_value = self.substitute_user_function_argument_bindings(
            &returned_value,
            outer_user_function,
            outer_arguments,
        );
        let Expression::Identifier(inner_name) = substituted_value else {
            return None;
        };
        let inner_user_function = self.module.user_function_map.get(&inner_name)?;
        let summary = inner_user_function.inline_summary.as_ref()?;
        if !summary.effects.is_empty() {
            return None;
        }
        let return_value = summary.return_value.as_ref()?;
        let outer_substituted_return = self.substitute_user_function_argument_bindings(
            return_value,
            outer_user_function,
            outer_arguments,
        );

        Some(self.substitute_user_function_argument_bindings(
            &outer_substituted_return,
            inner_user_function,
            arguments,
        ))
    }
}
