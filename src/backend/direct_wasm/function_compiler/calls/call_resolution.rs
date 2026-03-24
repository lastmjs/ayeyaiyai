use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_bound_snapshot_binding_name<'b>(
        &self,
        name: &'b str,
        bindings: &HashMap<String, Expression>,
    ) -> &'b str {
        if bindings.contains_key(name) {
            return name;
        }
        scoped_binding_source_name(name)
            .filter(|source_name| bindings.contains_key(*source_name))
            .unwrap_or(name)
    }

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
        if let Some(object_binding) =
            self.resolve_returned_object_binding_from_call(callee, arguments)
            && let Some(value) = object_binding_lookup_value(
                &object_binding,
                &Expression::String(property_name.clone()),
            )
        {
            return Some(value.clone());
        }
        let LocalFunctionBinding::User(function_name) =
            self.resolve_function_binding_from_expression(callee)?
        else {
            return None;
        };
        let user_function = self.module.user_function_map.get(&function_name)?;
        let binding = user_function
            .returned_member_value_bindings
            .iter()
            .find(|binding| binding.property == *property_name)?;

        let mut value = self.substitute_user_function_argument_bindings(
            &binding.value,
            user_function,
            arguments,
        );
        if let Expression::Member { object, property } = callee
            && let Some(capture_slots) =
                self.resolve_member_function_capture_slots(object, property)
        {
            value = self.substitute_capture_slot_bindings(&value, &capture_slots);
        }

        Some(value)
    }

    pub(in crate::backend::direct_wasm) fn resolve_returned_object_binding_from_call(
        &self,
        callee: &Expression,
        arguments: &[CallArgument],
    ) -> Option<ObjectValueBinding> {
        let LocalFunctionBinding::User(function_name) =
            self.resolve_function_binding_from_expression(callee)?
        else {
            return None;
        };
        if let Some(snapshot) = self
            .last_bound_user_function_call
            .as_ref()
            .filter(|snapshot| snapshot.function_name == function_name)
            && let Some(result) = snapshot.result_expression.as_ref()
            && let Some(object_binding) = self.resolve_object_binding_from_expression(&result)
        {
            return Some(object_binding);
        }
        let user_function = self.module.user_function_map.get(&function_name)?;
        if user_function.returned_member_value_bindings.is_empty() {
            return None;
        }
        let capture_bindings = match callee {
            Expression::Member { object, property } => self
                .resolve_member_function_capture_slots(object, property)
                .unwrap_or_default(),
            _ => BTreeMap::new(),
        };
        let mut object_binding = empty_object_value_binding();
        for binding in &user_function.returned_member_value_bindings {
            let mut value = self.substitute_user_function_argument_bindings(
                &binding.value,
                user_function,
                arguments,
            );
            if !capture_bindings.is_empty() {
                value = self.substitute_capture_slot_bindings(&value, &capture_bindings);
            }
            object_binding_set_property(
                &mut object_binding,
                Expression::String(binding.property.clone()),
                value,
            );
        }
        Some(object_binding)
    }

    pub(in crate::backend::direct_wasm) fn resolve_bound_snapshot_user_function_result(
        &self,
        function_name: &str,
        bindings: &HashMap<String, Expression>,
    ) -> Option<(Expression, HashMap<String, Expression>)> {
        self.resolve_bound_snapshot_user_function_result_with_arguments(
            function_name,
            bindings,
            &[],
        )
    }

    pub(in crate::backend::direct_wasm) fn resolve_bound_snapshot_user_function_result_with_arguments(
        &self,
        function_name: &str,
        bindings: &HashMap<String, Expression>,
        arguments: &[Expression],
    ) -> Option<(Expression, HashMap<String, Expression>)> {
        let function = self.resolve_registered_function_declaration(function_name)?;
        let user_function = self.module.user_function_map.get(function_name)?;
        if user_function.has_parameter_defaults() {
            return None;
        }
        if !user_function.params.is_empty()
            && (!user_function.extra_argument_indices.is_empty()
                || arguments.len() > user_function.params.len())
        {
            return None;
        }
        let mut local_bindings = bindings.clone();
        for (index, parameter_name) in user_function.params.iter().enumerate() {
            local_bindings.insert(
                parameter_name.clone(),
                arguments
                    .get(index)
                    .cloned()
                    .unwrap_or(Expression::Undefined),
            );
        }
        let arguments_shadowed = user_function.lexical_this
            || user_function.body_declares_arguments_binding
            || user_function.params.iter().any(|param| {
                param == "arguments"
                    || scoped_binding_source_name(param)
                        .is_some_and(|source_name| source_name == "arguments")
            });
        if !arguments_shadowed {
            local_bindings.insert(
                "arguments".to_string(),
                Expression::Array(
                    arguments
                        .iter()
                        .cloned()
                        .map(ArrayElement::Expression)
                        .collect(),
                ),
            );
        }
        let result = self
            .execute_bound_snapshot_statements(
                &function.body,
                &mut local_bindings,
                Some(function_name),
            )?
            .unwrap_or(Expression::Undefined);
        Some((result, local_bindings))
    }

    pub(in crate::backend::direct_wasm) fn execute_bound_snapshot_statements(
        &self,
        statements: &[Statement],
        bindings: &mut HashMap<String, Expression>,
        current_function_name: Option<&str>,
    ) -> Option<Option<Expression>> {
        for statement in statements {
            match statement {
                Statement::Block { body } => {
                    if let Some(Some(result)) = self.execute_bound_snapshot_statements(
                        body,
                        bindings,
                        current_function_name,
                    ) {
                        return Some(Some(result));
                    }
                }
                Statement::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    let condition = self.evaluate_bound_snapshot_expression(
                        condition,
                        bindings,
                        current_function_name,
                    )?;
                    let branch = if matches!(condition, Expression::Bool(true)) {
                        then_branch
                    } else if matches!(condition, Expression::Bool(false)) {
                        else_branch
                    } else {
                        return None;
                    };
                    if let Some(Some(result)) = self.execute_bound_snapshot_statements(
                        branch,
                        bindings,
                        current_function_name,
                    ) {
                        return Some(Some(result));
                    }
                }
                Statement::Return(value) => {
                    return Some(Some(self.evaluate_bound_snapshot_expression(
                        value,
                        bindings,
                        current_function_name,
                    )?));
                }
                Statement::Throw(value) => {
                    self.evaluate_bound_snapshot_expression(
                        value,
                        bindings,
                        current_function_name,
                    )?;
                    return Some(Some(Expression::Undefined));
                }
                Statement::Var { name, value }
                | Statement::Let { name, value, .. }
                | Statement::Assign { name, value } => {
                    let resolved_name = self
                        .resolve_bound_snapshot_binding_name(name, bindings)
                        .to_string();
                    let value = self.evaluate_bound_snapshot_expression(
                        value,
                        bindings,
                        current_function_name,
                    )?;
                    bindings.insert(resolved_name, value);
                }
                Statement::AssignMember {
                    object,
                    property,
                    value,
                } => {
                    self.apply_bound_snapshot_member_assignment(
                        object,
                        property,
                        value,
                        bindings,
                        current_function_name,
                    )?;
                }
                Statement::Expression(expression) => {
                    self.evaluate_bound_snapshot_expression(
                        expression,
                        bindings,
                        current_function_name,
                    )?;
                }
                Statement::Print { values } => {
                    for value in values {
                        self.evaluate_bound_snapshot_expression(
                            value,
                            bindings,
                            current_function_name,
                        )?;
                    }
                }
                _ => return None,
            }
        }
        Some(None)
    }

    pub(in crate::backend::direct_wasm) fn apply_bound_snapshot_member_assignment(
        &self,
        object: &Expression,
        property: &Expression,
        value: &Expression,
        bindings: &mut HashMap<String, Expression>,
        current_function_name: Option<&str>,
    ) -> Option<Expression> {
        let Expression::Identifier(object_name) = object else {
            return None;
        };
        let object_name = self
            .resolve_bound_snapshot_binding_name(object_name, bindings)
            .to_string();
        let property =
            self.evaluate_bound_snapshot_expression(property, bindings, current_function_name)?;
        let value =
            self.evaluate_bound_snapshot_expression(value, bindings, current_function_name)?;
        let current_object = bindings
            .get(&object_name)
            .cloned()
            .unwrap_or_else(|| Expression::Identifier(object_name.clone()));
        let mut object_binding = self.resolve_object_binding_from_expression(&current_object)?;
        object_binding_set_property(&mut object_binding, property, value.clone());
        bindings.insert(
            object_name.clone(),
            object_binding_to_expression(&object_binding),
        );
        Some(value)
    }

    fn apply_bound_snapshot_user_function_call_effects(
        &self,
        function_name: &str,
        arguments: &[Expression],
        bindings: &mut HashMap<String, Expression>,
        current_function_name: Option<&str>,
    ) -> Option<Expression> {
        let user_function = self.module.user_function_map.get(function_name)?;
        if user_function.is_async() || user_function.is_generator() {
            return None;
        }
        let evaluated_arguments = arguments
            .iter()
            .map(|argument| {
                self.evaluate_bound_snapshot_expression(argument, bindings, current_function_name)
            })
            .collect::<Option<Vec<_>>>()?;
        let (result, updated_bindings) = self
            .resolve_bound_snapshot_user_function_result_with_arguments(
                function_name,
                bindings,
                &evaluated_arguments,
            )?;
        for (name, value) in updated_bindings {
            let source_name = scoped_binding_source_name(&name)
                .unwrap_or(&name)
                .to_string();
            if user_function.scope_bindings.contains(&source_name) {
                continue;
            }
            bindings.insert(source_name, value);
        }
        Some(result)
    }

    pub(in crate::backend::direct_wasm) fn evaluate_bound_snapshot_expression(
        &self,
        expression: &Expression,
        bindings: &mut HashMap<String, Expression>,
        current_function_name: Option<&str>,
    ) -> Option<Expression> {
        match expression {
            Expression::Identifier(name) => {
                let resolved_name = self.resolve_bound_snapshot_binding_name(name, bindings);
                Some(
                    bindings
                        .get(resolved_name)
                        .cloned()
                        .unwrap_or_else(|| expression.clone()),
                )
            }
            Expression::Number(_)
            | Expression::BigInt(_)
            | Expression::String(_)
            | Expression::Bool(_)
            | Expression::Null
            | Expression::Undefined => Some(expression.clone()),
            Expression::Binary { op, left, right } => {
                let left =
                    self.evaluate_bound_snapshot_expression(left, bindings, current_function_name)?;
                let right = self.evaluate_bound_snapshot_expression(
                    right,
                    bindings,
                    current_function_name,
                )?;
                match op {
                    BinaryOp::Add => match (&left, &right) {
                        (Expression::Number(lhs), Expression::Number(rhs)) => {
                            Some(Expression::Number(lhs + rhs))
                        }
                        (Expression::String(lhs), Expression::String(rhs)) => {
                            Some(Expression::String(format!("{lhs}{rhs}")))
                        }
                        _ => None,
                    },
                    BinaryOp::GreaterThanOrEqual => match (&left, &right) {
                        (Expression::Number(lhs), Expression::Number(rhs)) => {
                            Some(Expression::Bool(lhs >= rhs))
                        }
                        _ => None,
                    },
                    _ => None,
                }
            }
            Expression::Member { object, property } => {
                let object = self.evaluate_bound_snapshot_expression(
                    object,
                    bindings,
                    current_function_name,
                )?;
                let property = self.evaluate_bound_snapshot_expression(
                    property,
                    bindings,
                    current_function_name,
                )?;
                match (object, property) {
                    (Expression::Array(elements), Expression::String(name)) if name == "length" => {
                        Some(Expression::Number(elements.len() as f64))
                    }
                    (Expression::Array(elements), Expression::Number(index))
                        if index.is_finite() && index.fract() == 0.0 && index >= 0.0 =>
                    {
                        let index = index as usize;
                        match elements.get(index) {
                            Some(ArrayElement::Expression(value)) => Some(value.clone()),
                            Some(ArrayElement::Spread(_)) => None,
                            None => Some(Expression::Undefined),
                        }
                    }
                    (Expression::Object(entries), Expression::String(name)) => entries
                        .into_iter()
                        .find_map(|entry| match entry {
                            ObjectEntry::Data {
                                key: Expression::String(property_name),
                                value,
                            } if property_name == name => Some(value),
                            _ => None,
                        })
                        .or(Some(Expression::Undefined)),
                    _ => None,
                }
            }
            Expression::Assign { name, value } => {
                let resolved_name = self
                    .resolve_bound_snapshot_binding_name(name, bindings)
                    .to_string();
                let value = self.evaluate_bound_snapshot_expression(
                    value,
                    bindings,
                    current_function_name,
                )?;
                bindings.insert(resolved_name, value.clone());
                Some(value)
            }
            Expression::AssignMember {
                object,
                property,
                value,
            } => {
                if let Some(LocalFunctionBinding::User(function_name)) =
                    self.resolve_member_setter_binding(object, property)
                {
                    let argument = self.evaluate_bound_snapshot_expression(
                        value,
                        bindings,
                        current_function_name,
                    )?;
                    self.apply_bound_snapshot_user_function_call_effects(
                        &function_name,
                        &[argument.clone()],
                        bindings,
                        current_function_name,
                    )?;
                    return Some(argument);
                }
                self.apply_bound_snapshot_member_assignment(
                    object,
                    property,
                    value,
                    bindings,
                    current_function_name,
                )
            }
            Expression::AssignSuperMember { property, value } => {
                let effective_property = self.resolve_property_key_expression(property)?;
                if let Some((_, binding)) =
                    self.resolve_super_runtime_prototype_binding_with_context(current_function_name)
                {
                    let variants =
                        self.resolve_user_super_setter_variants(&binding, &effective_property)?;
                    let argument = self.evaluate_bound_snapshot_expression(
                        value,
                        bindings,
                        current_function_name,
                    )?;
                    for (user_function, _) in variants {
                        self.apply_bound_snapshot_user_function_call_effects(
                            &user_function.name,
                            &[argument.clone()],
                            bindings,
                            current_function_name,
                        )?;
                    }
                    return Some(argument);
                }
                let super_base =
                    self.resolve_super_base_expression_with_context(current_function_name)?;
                let LocalFunctionBinding::User(function_name) =
                    self.resolve_member_setter_binding(&super_base, &effective_property)?
                else {
                    return None;
                };
                let argument = self.evaluate_bound_snapshot_expression(
                    value,
                    bindings,
                    current_function_name,
                )?;
                self.apply_bound_snapshot_user_function_call_effects(
                    &function_name,
                    &[argument.clone()],
                    bindings,
                    current_function_name,
                )?;
                Some(argument)
            }
            Expression::Call { callee, arguments }
            | Expression::SuperCall { callee, arguments }
            | Expression::New { callee, arguments } => {
                let LocalFunctionBinding::User(function_name) = self
                    .resolve_function_binding_from_expression_with_context(
                        callee,
                        current_function_name,
                    )?
                else {
                    return None;
                };
                self.apply_bound_snapshot_user_function_call_effects(
                    &function_name,
                    &arguments
                        .iter()
                        .map(|argument| match argument {
                            CallArgument::Expression(expression)
                            | CallArgument::Spread(expression) => expression.clone(),
                        })
                        .collect::<Vec<_>>(),
                    bindings,
                    current_function_name,
                )
            }
            Expression::Object(entries) => Some(Expression::Object(
                entries
                    .iter()
                    .map(|entry| match entry {
                        ObjectEntry::Data { key, value } => Some(ObjectEntry::Data {
                            key: self.evaluate_bound_snapshot_expression(
                                key,
                                bindings,
                                current_function_name,
                            )?,
                            value: self.evaluate_bound_snapshot_expression(
                                value,
                                bindings,
                                current_function_name,
                            )?,
                        }),
                        _ => None,
                    })
                    .collect::<Option<Vec<_>>>()?,
            )),
            Expression::Update { name, op, prefix } => {
                let resolved_name = self
                    .resolve_bound_snapshot_binding_name(name, bindings)
                    .to_string();
                let current = bindings.get(&resolved_name)?.clone();
                let Expression::Number(current_number) = current else {
                    return None;
                };
                let next_number = match op {
                    UpdateOp::Increment => current_number + 1.0,
                    UpdateOp::Decrement => current_number - 1.0,
                };
                bindings.insert(resolved_name, Expression::Number(next_number));
                Some(if *prefix {
                    Expression::Number(next_number)
                } else {
                    Expression::Number(current_number)
                })
            }
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn substitute_capture_slot_bindings(
        &self,
        expression: &Expression,
        bindings: &BTreeMap<String, String>,
    ) -> Expression {
        match expression {
            Expression::Identifier(name) => bindings
                .get(name)
                .cloned()
                .map(Expression::Identifier)
                .unwrap_or_else(|| expression.clone()),
            Expression::Member { object, property } => Expression::Member {
                object: Box::new(self.substitute_capture_slot_bindings(object, bindings)),
                property: Box::new(self.substitute_capture_slot_bindings(property, bindings)),
            },
            Expression::Assign { name, value } => Expression::Assign {
                name: bindings.get(name).cloned().unwrap_or_else(|| name.clone()),
                value: Box::new(self.substitute_capture_slot_bindings(value, bindings)),
            },
            Expression::AssignMember {
                object,
                property,
                value,
            } => Expression::AssignMember {
                object: Box::new(self.substitute_capture_slot_bindings(object, bindings)),
                property: Box::new(self.substitute_capture_slot_bindings(property, bindings)),
                value: Box::new(self.substitute_capture_slot_bindings(value, bindings)),
            },
            Expression::AssignSuperMember { property, value } => Expression::AssignSuperMember {
                property: Box::new(self.substitute_capture_slot_bindings(property, bindings)),
                value: Box::new(self.substitute_capture_slot_bindings(value, bindings)),
            },
            Expression::Await(value) => Expression::Await(Box::new(
                self.substitute_capture_slot_bindings(value, bindings),
            )),
            Expression::EnumerateKeys(value) => Expression::EnumerateKeys(Box::new(
                self.substitute_capture_slot_bindings(value, bindings),
            )),
            Expression::GetIterator(value) => Expression::GetIterator(Box::new(
                self.substitute_capture_slot_bindings(value, bindings),
            )),
            Expression::IteratorClose(value) => Expression::IteratorClose(Box::new(
                self.substitute_capture_slot_bindings(value, bindings),
            )),
            Expression::Unary { op, expression } => Expression::Unary {
                op: *op,
                expression: Box::new(self.substitute_capture_slot_bindings(expression, bindings)),
            },
            Expression::Binary { op, left, right } => Expression::Binary {
                op: *op,
                left: Box::new(self.substitute_capture_slot_bindings(left, bindings)),
                right: Box::new(self.substitute_capture_slot_bindings(right, bindings)),
            },
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => Expression::Conditional {
                condition: Box::new(self.substitute_capture_slot_bindings(condition, bindings)),
                then_expression: Box::new(
                    self.substitute_capture_slot_bindings(then_expression, bindings),
                ),
                else_expression: Box::new(
                    self.substitute_capture_slot_bindings(else_expression, bindings),
                ),
            },
            Expression::Sequence(expressions) => Expression::Sequence(
                expressions
                    .iter()
                    .map(|expression| self.substitute_capture_slot_bindings(expression, bindings))
                    .collect(),
            ),
            Expression::Call { callee, arguments } => Expression::Call {
                callee: Box::new(self.substitute_capture_slot_bindings(callee, bindings)),
                arguments: arguments
                    .iter()
                    .map(|argument| match argument {
                        CallArgument::Expression(expression) => CallArgument::Expression(
                            self.substitute_capture_slot_bindings(expression, bindings),
                        ),
                        CallArgument::Spread(expression) => CallArgument::Spread(
                            self.substitute_capture_slot_bindings(expression, bindings),
                        ),
                    })
                    .collect(),
            },
            Expression::SuperCall { callee, arguments } => Expression::SuperCall {
                callee: Box::new(self.substitute_capture_slot_bindings(callee, bindings)),
                arguments: arguments
                    .iter()
                    .map(|argument| match argument {
                        CallArgument::Expression(expression) => CallArgument::Expression(
                            self.substitute_capture_slot_bindings(expression, bindings),
                        ),
                        CallArgument::Spread(expression) => CallArgument::Spread(
                            self.substitute_capture_slot_bindings(expression, bindings),
                        ),
                    })
                    .collect(),
            },
            Expression::New { callee, arguments } => Expression::New {
                callee: Box::new(self.substitute_capture_slot_bindings(callee, bindings)),
                arguments: arguments
                    .iter()
                    .map(|argument| match argument {
                        CallArgument::Expression(expression) => CallArgument::Expression(
                            self.substitute_capture_slot_bindings(expression, bindings),
                        ),
                        CallArgument::Spread(expression) => CallArgument::Spread(
                            self.substitute_capture_slot_bindings(expression, bindings),
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
                                self.substitute_capture_slot_bindings(expression, bindings),
                            )
                        }
                        crate::ir::hir::ArrayElement::Spread(expression) => {
                            crate::ir::hir::ArrayElement::Spread(
                                self.substitute_capture_slot_bindings(expression, bindings),
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
                                key: self.substitute_capture_slot_bindings(key, bindings),
                                value: self.substitute_capture_slot_bindings(value, bindings),
                            }
                        }
                        crate::ir::hir::ObjectEntry::Getter { key, getter } => {
                            crate::ir::hir::ObjectEntry::Getter {
                                key: self.substitute_capture_slot_bindings(key, bindings),
                                getter: self.substitute_capture_slot_bindings(getter, bindings),
                            }
                        }
                        crate::ir::hir::ObjectEntry::Setter { key, setter } => {
                            crate::ir::hir::ObjectEntry::Setter {
                                key: self.substitute_capture_slot_bindings(key, bindings),
                                setter: self.substitute_capture_slot_bindings(setter, bindings),
                            }
                        }
                        crate::ir::hir::ObjectEntry::Spread(expression) => {
                            crate::ir::hir::ObjectEntry::Spread(
                                self.substitute_capture_slot_bindings(expression, bindings),
                            )
                        }
                    })
                    .collect(),
            ),
            Expression::Update { name, op, prefix } => Expression::Update {
                name: bindings.get(name).cloned().unwrap_or_else(|| name.clone()),
                op: *op,
                prefix: *prefix,
            },
            _ => expression.clone(),
        }
    }

    pub(in crate::backend::direct_wasm) fn substitute_user_function_argument_bindings(
        &self,
        expression: &Expression,
        user_function: &UserFunction,
        arguments: &[CallArgument],
    ) -> Expression {
        let expanded_arguments = self
            .expand_call_arguments(arguments)
            .into_iter()
            .map(CallArgument::Expression)
            .collect::<Vec<_>>();
        let mut bindings = HashMap::new();
        for (index, param_name) in user_function.params.iter().enumerate() {
            let value = match expanded_arguments.get(index) {
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
        let self_binding_name = self
            .resolve_registered_function_declaration(&user_function.name)
            .and_then(|function| function.self_binding.as_deref());
        match expression {
            Expression::This if !user_function.lexical_this => this_binding.clone(),
            Expression::Identifier(name) if name == "arguments" && !arguments_shadowed => {
                arguments_binding.clone()
            }
            Expression::Identifier(name)
                if self_binding_name.is_some_and(|self_binding| name == self_binding) =>
            {
                Expression::Identifier(user_function.name.clone())
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
        if let Some(summary) = user_function.inline_summary.as_ref()
            && let Some(return_value) = summary.return_value.as_ref()
        {
            return Some(self.substitute_user_function_call_frame_bindings(
                return_value,
                user_function,
                &call_arguments,
                this_binding,
                &arguments_binding,
            ));
        }

        let function = self.resolve_registered_function_declaration(function_name)?;
        let (terminal_statement, effect_statements) = function.body.split_last()?;
        if !effect_statements
            .iter()
            .all(|statement| matches!(statement, Statement::Block { body } if body.is_empty()))
        {
            return None;
        }
        let Statement::Return(return_value) = terminal_statement else {
            return None;
        };
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
