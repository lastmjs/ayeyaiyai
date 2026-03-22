use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn infer_global_arguments_binding(
        &self,
        expression: &Expression,
    ) -> Option<ArgumentsValueBinding> {
        match expression {
            Expression::Identifier(name) => self.global_arguments_bindings.get(name).cloned(),
            Expression::Call { callee, arguments } | Expression::New { callee, arguments } => {
                let Expression::Identifier(name) = callee.as_ref() else {
                    return None;
                };
                let user_function = if let Some(LocalFunctionBinding::User(function_name)) =
                    self.global_function_bindings.get(name)
                {
                    self.user_function_map.get(function_name)
                } else if is_internal_user_function_identifier(name) {
                    self.user_function_map.get(name)
                } else {
                    None
                }?;
                if !user_function.returns_arguments_object {
                    return None;
                }
                Some(ArgumentsValueBinding::for_user_function(
                    user_function,
                    expand_static_call_arguments(arguments, &self.global_array_bindings),
                ))
            }
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn infer_global_array_binding(
        &self,
        expression: &Expression,
    ) -> Option<ArrayValueBinding> {
        match expression {
            Expression::Identifier(name) => self.global_array_bindings.get(name).cloned(),
            Expression::EnumerateKeys(value) => self.infer_enumerated_keys_binding(value),
            Expression::Call { callee, arguments } => {
                if let Some(binding) =
                    self.infer_global_builtin_array_call_binding(callee, arguments)
                {
                    return Some(binding);
                }
                let Expression::Identifier(name) = callee.as_ref() else {
                    return None;
                };
                let user_function = if let Some(LocalFunctionBinding::User(function_name)) =
                    self.global_function_bindings.get(name)
                {
                    self.user_function_map.get(function_name)
                } else if is_internal_user_function_identifier(name) {
                    self.user_function_map.get(name)
                } else {
                    None
                }?;
                let param_index = user_function.enumerated_keys_param_index?;
                let argument = match arguments.get(param_index) {
                    Some(CallArgument::Expression(expression))
                    | Some(CallArgument::Spread(expression)) => expression,
                    None => return Some(ArrayValueBinding { values: Vec::new() }),
                };
                self.infer_enumerated_keys_binding(argument)
            }
            Expression::New { callee, arguments } => {
                let Expression::Identifier(name) = callee.as_ref() else {
                    return None;
                };
                let user_function = if let Some(LocalFunctionBinding::User(function_name)) =
                    self.global_function_bindings.get(name)
                {
                    self.user_function_map.get(function_name)
                } else if is_internal_user_function_identifier(name) {
                    self.user_function_map.get(name)
                } else {
                    None
                }?;
                let param_index = user_function.enumerated_keys_param_index?;
                let argument = match arguments.get(param_index) {
                    Some(CallArgument::Expression(expression))
                    | Some(CallArgument::Spread(expression)) => expression,
                    None => return Some(ArrayValueBinding { values: Vec::new() }),
                };
                self.infer_enumerated_keys_binding(argument)
            }
            Expression::Array(elements) => {
                let mut values = Vec::new();
                for element in elements {
                    match element {
                        crate::ir::hir::ArrayElement::Expression(expression) => {
                            values.push(Some(self.materialize_global_expression(expression)));
                        }
                        crate::ir::hir::ArrayElement::Spread(expression) => {
                            if let Some(binding) = self.infer_global_array_binding(expression) {
                                values.extend(binding.values);
                            } else {
                                values.push(Some(self.materialize_global_expression(expression)));
                            }
                        }
                    }
                }
                Some(ArrayValueBinding { values })
            }
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn infer_global_object_binding(
        &self,
        expression: &Expression,
    ) -> Option<ObjectValueBinding> {
        let mut value_bindings = self.global_value_bindings.clone();
        let mut object_bindings = self.global_object_bindings.clone();
        self.infer_global_object_binding_with_state(
            expression,
            &mut value_bindings,
            &mut object_bindings,
        )
    }

    pub(in crate::backend::direct_wasm) fn infer_global_object_binding_with_state(
        &self,
        expression: &Expression,
        value_bindings: &mut HashMap<String, Expression>,
        object_bindings: &mut HashMap<String, ObjectValueBinding>,
    ) -> Option<ObjectValueBinding> {
        match expression {
            Expression::Identifier(name) => object_bindings.get(name).cloned().or_else(|| {
                value_bindings
                    .get(name)
                    .cloned()
                    .filter(
                        |value| !matches!(value, Expression::Identifier(alias) if alias == name),
                    )
                    .and_then(|value| {
                        self.infer_global_object_binding_with_state(
                            &value,
                            value_bindings,
                            object_bindings,
                        )
                    })
            }),
            Expression::Object(entries) => {
                let mut object_binding = empty_object_value_binding();
                for entry in entries {
                    match entry {
                        crate::ir::hir::ObjectEntry::Data { key, value } => {
                            let materialized_key = self
                                .materialize_global_expression_with_state(
                                    key,
                                    &HashMap::new(),
                                    value_bindings,
                                    object_bindings,
                                )
                                .unwrap_or_else(|| self.materialize_global_expression(key));
                            let value = self
                                .materialize_global_expression_with_state(
                                    value,
                                    &HashMap::new(),
                                    value_bindings,
                                    object_bindings,
                                )
                                .unwrap_or_else(|| self.materialize_global_expression(value));
                            object_binding_set_property(
                                &mut object_binding,
                                materialized_key,
                                value,
                            );
                        }
                        crate::ir::hir::ObjectEntry::Getter { key, .. }
                        | crate::ir::hir::ObjectEntry::Setter { key, .. } => {
                            let materialized_key = self
                                .materialize_global_expression_with_state(
                                    key,
                                    &HashMap::new(),
                                    value_bindings,
                                    object_bindings,
                                )
                                .unwrap_or_else(|| self.materialize_global_expression(key));
                            object_binding_set_property(
                                &mut object_binding,
                                materialized_key,
                                Expression::Undefined,
                            );
                        }
                        crate::ir::hir::ObjectEntry::Spread(expression) => {
                            let spread_expression = self
                                .materialize_global_expression_with_state(
                                    expression,
                                    &HashMap::new(),
                                    value_bindings,
                                    object_bindings,
                                )
                                .unwrap_or_else(|| self.materialize_global_expression(expression));
                            if matches!(spread_expression, Expression::Null | Expression::Undefined)
                                || matches!(
                                    &spread_expression,
                                    Expression::Identifier(name)
                                        if name == "undefined"
                                            && !self.global_bindings.contains_key(name)
                                            && !self.global_lexical_bindings.contains(name)
                                )
                            {
                                continue;
                            }
                            let spread_binding = self
                                .infer_global_copy_data_properties_binding_with_state(
                                    &spread_expression,
                                    value_bindings,
                                    object_bindings,
                                )?;
                            merge_enumerable_object_binding(&mut object_binding, &spread_binding);
                        }
                    }
                }
                Some(object_binding)
            }
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn infer_global_copy_data_properties_binding_with_state(
        &self,
        expression: &Expression,
        value_bindings: &mut HashMap<String, Expression>,
        object_bindings: &mut HashMap<String, ObjectValueBinding>,
    ) -> Option<ObjectValueBinding> {
        let source_binding = self.infer_global_object_binding_with_state(
            expression,
            value_bindings,
            object_bindings,
        )?;
        let mut copied_binding = empty_object_value_binding();
        for (name, _) in &source_binding.string_properties {
            if source_binding
                .non_enumerable_string_properties
                .iter()
                .any(|hidden_name| hidden_name == name)
            {
                continue;
            }
            let property = Expression::String(name.clone());
            let copied_value = self
                .infer_global_member_getter_return_value_with_state(
                    expression,
                    &property,
                    value_bindings,
                    object_bindings,
                )
                .or_else(|| {
                    self.infer_global_object_binding_with_state(
                        expression,
                        value_bindings,
                        object_bindings,
                    )
                    .and_then(|binding| object_binding_lookup_value(&binding, &property).cloned())
                })
                .unwrap_or(Expression::Undefined);
            object_binding_set_property(&mut copied_binding, property, copied_value);
        }
        for (property, _) in &source_binding.symbol_properties {
            let copied_value = self
                .infer_global_member_getter_return_value_with_state(
                    expression,
                    property,
                    value_bindings,
                    object_bindings,
                )
                .or_else(|| {
                    self.infer_global_object_binding_with_state(
                        expression,
                        value_bindings,
                        object_bindings,
                    )
                    .and_then(|binding| object_binding_lookup_value(&binding, property).cloned())
                })
                .unwrap_or(Expression::Undefined);
            object_binding_set_property(&mut copied_binding, property.clone(), copied_value);
        }
        Some(copied_binding)
    }

    #[cfg(test)]

    pub(in crate::backend::direct_wasm) fn infer_global_copy_data_properties_binding(
        &self,
        expression: &Expression,
    ) -> Option<ObjectValueBinding> {
        let mut value_bindings = self.global_value_bindings.clone();
        let mut object_bindings = self.global_object_bindings.clone();
        self.infer_global_copy_data_properties_binding_with_state(
            expression,
            &mut value_bindings,
            &mut object_bindings,
        )
    }

    pub(in crate::backend::direct_wasm) fn infer_global_member_getter_return_value_with_state(
        &self,
        object: &Expression,
        property: &Expression,
        value_bindings: &mut HashMap<String, Expression>,
        object_bindings: &mut HashMap<String, ObjectValueBinding>,
    ) -> Option<Expression> {
        let getter_binding = self.infer_global_member_getter_binding(object, property)?;
        self.execute_global_function_binding_with_state(
            &getter_binding,
            &[],
            value_bindings,
            object_bindings,
        )
    }

    #[cfg(test)]

    pub(in crate::backend::direct_wasm) fn infer_global_member_getter_return_value(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> Option<Expression> {
        let mut value_bindings = self.global_value_bindings.clone();
        let mut object_bindings = self.global_object_bindings.clone();
        self.infer_global_member_getter_return_value_with_state(
            object,
            property,
            &mut value_bindings,
            &mut object_bindings,
        )
    }

    pub(in crate::backend::direct_wasm) fn infer_global_member_getter_binding(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> Option<LocalFunctionBinding> {
        let target = match object {
            Expression::Identifier(name) => MemberFunctionBindingTarget::Identifier(name.clone()),
            Expression::Member { object, property } if matches!(property.as_ref(), Expression::String(name) if name == "prototype") =>
            {
                let Expression::Identifier(name) = object.as_ref() else {
                    return None;
                };
                MemberFunctionBindingTarget::Prototype(name.clone())
            }
            Expression::New { callee, .. } => {
                let Expression::Identifier(name) = callee.as_ref() else {
                    return None;
                };
                MemberFunctionBindingTarget::Prototype(name.clone())
            }
            _ => return None,
        };
        let property = if let Some(property_name) = static_property_name_from_expression(property) {
            MemberFunctionBindingProperty::String(property_name)
        } else {
            return None;
        };
        let key = MemberFunctionBindingKey { target, property };
        self.global_member_getter_bindings.get(&key).cloned()
    }

    pub(in crate::backend::direct_wasm) fn infer_global_member_function_binding_property(
        &self,
        property: &Expression,
    ) -> Option<MemberFunctionBindingProperty> {
        let resolved_property = self.materialize_global_expression(property);
        static_property_name_from_expression(&resolved_property)
            .map(MemberFunctionBindingProperty::String)
    }

    pub(in crate::backend::direct_wasm) fn execute_global_function_binding_with_state(
        &self,
        binding: &LocalFunctionBinding,
        arguments: &[CallArgument],
        value_bindings: &mut HashMap<String, Expression>,
        object_bindings: &mut HashMap<String, ObjectValueBinding>,
    ) -> Option<Expression> {
        let LocalFunctionBinding::User(function_name) = binding else {
            return None;
        };
        let user_function = self.user_function_map.get(function_name)?;
        if let Some(summary) = user_function.inline_summary.as_ref()
            && summary.effects.is_empty()
            && let Some(return_value) = summary.return_value.as_ref()
        {
            let substituted = self.substitute_global_user_function_argument_bindings(
                return_value,
                user_function,
                arguments,
            );
            if let Some(materialized) = self.materialize_global_expression_with_state(
                &substituted,
                &HashMap::new(),
                value_bindings,
                object_bindings,
            ) {
                return Some(materialized);
            }
        }

        let function = self
            .registered_function_declarations
            .iter()
            .find(|function| function.name == *function_name)?;
        let mut local_bindings = HashMap::new();
        for statement in &function.body {
            match statement {
                Statement::Var { name, value } | Statement::Let { name, value, .. } => {
                    let value = self.evaluate_global_expression_with_state(
                        value,
                        &mut local_bindings,
                        value_bindings,
                        object_bindings,
                    )?;
                    local_bindings.insert(name.clone(), value);
                }
                Statement::Assign { name, value } => {
                    let value = self.evaluate_global_expression_with_state(
                        value,
                        &mut local_bindings,
                        value_bindings,
                        object_bindings,
                    )?;
                    self.assign_global_expression_with_state(
                        name,
                        value,
                        &mut local_bindings,
                        value_bindings,
                        object_bindings,
                    )?;
                }
                Statement::AssignMember {
                    object,
                    property,
                    value,
                } => {
                    let property = self.evaluate_global_expression_with_state(
                        property,
                        &mut local_bindings,
                        value_bindings,
                        object_bindings,
                    )?;
                    let value = self.evaluate_global_expression_with_state(
                        value,
                        &mut local_bindings,
                        value_bindings,
                        object_bindings,
                    )?;
                    self.assign_global_member_expression_with_state(
                        object,
                        property,
                        value,
                        &mut local_bindings,
                        value_bindings,
                        object_bindings,
                    )?;
                }
                Statement::Expression(expression) => {
                    self.evaluate_global_expression_with_state(
                        expression,
                        &mut local_bindings,
                        value_bindings,
                        object_bindings,
                    )?;
                }
                Statement::Return(expression) => {
                    return self.evaluate_global_expression_with_state(
                        expression,
                        &mut local_bindings,
                        value_bindings,
                        object_bindings,
                    );
                }
                Statement::Block { body } if body.is_empty() => {}
                _ => return None,
            }
        }

        Some(Expression::Undefined)
    }

    pub(in crate::backend::direct_wasm) fn materialize_global_expression_with_state(
        &self,
        expression: &Expression,
        local_bindings: &HashMap<String, Expression>,
        value_bindings: &HashMap<String, Expression>,
        object_bindings: &HashMap<String, ObjectValueBinding>,
    ) -> Option<Expression> {
        match expression {
            Expression::Identifier(name) => {
                if self.global_kinds.get(name) == Some(&StaticValueKind::Symbol) {
                    return Some(Expression::Identifier(name.clone()));
                }
                if value_bindings.get(name).is_some_and(|value| {
                    matches!(
                        value,
                        Expression::Call { callee, .. }
                            if matches!(callee.as_ref(), Expression::Identifier(symbol_name)
                                if symbol_name == "Symbol"
                                    && !self.global_bindings.contains_key(symbol_name)
                                    && !self.global_lexical_bindings.contains(symbol_name))
                    )
                }) {
                    return Some(Expression::Identifier(name.clone()));
                }
                if let Some(value) = local_bindings.get(name) {
                    return self.materialize_global_expression_with_state(
                        value,
                        local_bindings,
                        value_bindings,
                        object_bindings,
                    );
                }
                if let Some(value) = value_bindings.get(name) {
                    if object_bindings.contains_key(name)
                        && matches!(value, Expression::Object(_) | Expression::Identifier(_))
                    {
                        return Some(Expression::Identifier(name.clone()));
                    }
                    if !matches!(value, Expression::Identifier(alias) if alias == name) {
                        return self.materialize_global_expression_with_state(
                            value,
                            local_bindings,
                            value_bindings,
                            object_bindings,
                        );
                    }
                }
                Some(expression.clone())
            }
            Expression::Number(_)
            | Expression::BigInt(_)
            | Expression::String(_)
            | Expression::Bool(_)
            | Expression::Null
            | Expression::Undefined
            | Expression::This
            | Expression::NewTarget
            | Expression::Sent => Some(expression.clone()),
            Expression::Member { object, property } => {
                let object_binding = self.resolve_stateful_object_binding_from_expression(
                    object,
                    local_bindings,
                    value_bindings,
                    object_bindings,
                )?;
                let property = self.materialize_global_expression_with_state(
                    property,
                    local_bindings,
                    value_bindings,
                    object_bindings,
                )?;
                if let Some(value) = object_binding_lookup_value(&object_binding, &property) {
                    return self.materialize_global_expression_with_state(
                        value,
                        local_bindings,
                        value_bindings,
                        object_bindings,
                    );
                }
                if static_property_name_from_expression(&property).is_some()
                    || object_binding_has_property(&object_binding, &property)
                {
                    return Some(Expression::Undefined);
                }
                None
            }
            Expression::Object(entries) => Some(Expression::Object(
                entries
                    .iter()
                    .map(|entry| match entry {
                        ObjectEntry::Data { key, value } => Some(ObjectEntry::Data {
                            key: self.materialize_global_expression_with_state(
                                key,
                                local_bindings,
                                value_bindings,
                                object_bindings,
                            )?,
                            value: self.materialize_global_expression_with_state(
                                value,
                                local_bindings,
                                value_bindings,
                                object_bindings,
                            )?,
                        }),
                        _ => None,
                    })
                    .collect::<Option<Vec<_>>>()?,
            )),
            Expression::Array(elements) => Some(Expression::Array(
                elements
                    .iter()
                    .map(|element| match element {
                        ArrayElement::Expression(expression) => Some(ArrayElement::Expression(
                            self.materialize_global_expression_with_state(
                                expression,
                                local_bindings,
                                value_bindings,
                                object_bindings,
                            )?,
                        )),
                        _ => None,
                    })
                    .collect::<Option<Vec<_>>>()?,
            )),
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn evaluate_global_expression_with_state(
        &self,
        expression: &Expression,
        local_bindings: &mut HashMap<String, Expression>,
        value_bindings: &mut HashMap<String, Expression>,
        object_bindings: &mut HashMap<String, ObjectValueBinding>,
    ) -> Option<Expression> {
        match expression {
            Expression::Assign { name, value } => {
                let value = self.evaluate_global_expression_with_state(
                    value,
                    local_bindings,
                    value_bindings,
                    object_bindings,
                )?;
                self.assign_global_expression_with_state(
                    name,
                    value,
                    local_bindings,
                    value_bindings,
                    object_bindings,
                )
            }
            Expression::AssignMember {
                object,
                property,
                value,
            } => {
                let property = self.evaluate_global_expression_with_state(
                    property,
                    local_bindings,
                    value_bindings,
                    object_bindings,
                )?;
                let value = self.evaluate_global_expression_with_state(
                    value,
                    local_bindings,
                    value_bindings,
                    object_bindings,
                )?;
                self.assign_global_member_expression_with_state(
                    object,
                    property,
                    value,
                    local_bindings,
                    value_bindings,
                    object_bindings,
                )
            }
            Expression::Unary {
                op: UnaryOp::Delete,
                expression,
            } => match expression.as_ref() {
                Expression::Member { object, property } => {
                    let property = self.evaluate_global_expression_with_state(
                        property,
                        local_bindings,
                        value_bindings,
                        object_bindings,
                    )?;
                    let target_name = self.resolve_stateful_object_binding_name(
                        object,
                        local_bindings,
                        value_bindings,
                        object_bindings,
                    )?;
                    let binding = object_bindings.get_mut(&target_name)?;
                    object_binding_remove_property(binding, &property);
                    Some(Expression::Bool(true))
                }
                _ => Some(Expression::Bool(true)),
            },
            Expression::Update { name, op, prefix } => {
                let current = local_bindings
                    .get(name)
                    .cloned()
                    .or_else(|| value_bindings.get(name).cloned())
                    .unwrap_or(Expression::Undefined);
                let current_number = match current {
                    Expression::Number(value) => value,
                    Expression::Bool(true) => 1.0,
                    Expression::Bool(false) | Expression::Null => 0.0,
                    Expression::Undefined => f64::NAN,
                    _ => return None,
                };
                let next_number = match op {
                    UpdateOp::Increment => current_number + 1.0,
                    UpdateOp::Decrement => current_number - 1.0,
                };
                let next = Expression::Number(next_number);
                self.assign_global_expression_with_state(
                    name,
                    next.clone(),
                    local_bindings,
                    value_bindings,
                    object_bindings,
                )?;
                Some(if *prefix {
                    next
                } else {
                    Expression::Number(current_number)
                })
            }
            Expression::Sequence(expressions) => {
                let mut last = Expression::Undefined;
                for expression in expressions {
                    last = self.evaluate_global_expression_with_state(
                        expression,
                        local_bindings,
                        value_bindings,
                        object_bindings,
                    )?;
                }
                Some(last)
            }
            _ => self.materialize_global_expression_with_state(
                expression,
                local_bindings,
                value_bindings,
                object_bindings,
            ),
        }
    }

    pub(in crate::backend::direct_wasm) fn assign_global_expression_with_state(
        &self,
        name: &str,
        value: Expression,
        local_bindings: &mut HashMap<String, Expression>,
        value_bindings: &mut HashMap<String, Expression>,
        object_bindings: &mut HashMap<String, ObjectValueBinding>,
    ) -> Option<Expression> {
        if local_bindings.contains_key(name) {
            local_bindings.insert(name.to_string(), value.clone());
            return Some(value);
        }

        value_bindings.insert(name.to_string(), value.clone());
        if let Some(object_binding) =
            self.infer_global_object_binding_with_state(&value, value_bindings, object_bindings)
        {
            object_bindings.insert(name.to_string(), object_binding);
        } else {
            object_bindings.remove(name);
        }
        Some(value)
    }

    pub(in crate::backend::direct_wasm) fn assign_global_member_expression_with_state(
        &self,
        object: &Expression,
        property: Expression,
        value: Expression,
        local_bindings: &mut HashMap<String, Expression>,
        value_bindings: &mut HashMap<String, Expression>,
        object_bindings: &mut HashMap<String, ObjectValueBinding>,
    ) -> Option<Expression> {
        let target_name = self.resolve_stateful_object_binding_name(
            object,
            local_bindings,
            value_bindings,
            object_bindings,
        )?;
        let binding = object_bindings.get_mut(&target_name)?;
        object_binding_set_property(binding, property, value.clone());
        Some(value)
    }

    pub(in crate::backend::direct_wasm) fn resolve_stateful_object_binding_name(
        &self,
        expression: &Expression,
        local_bindings: &HashMap<String, Expression>,
        value_bindings: &HashMap<String, Expression>,
        object_bindings: &HashMap<String, ObjectValueBinding>,
    ) -> Option<String> {
        match expression {
            Expression::Identifier(name) if object_bindings.contains_key(name) => {
                Some(name.clone())
            }
            Expression::Identifier(name) => local_bindings
                .get(name)
                .or_else(|| value_bindings.get(name))
                .filter(|value| !matches!(value, Expression::Identifier(alias) if alias == name))
                .and_then(|value| {
                    self.resolve_stateful_object_binding_name(
                        value,
                        local_bindings,
                        value_bindings,
                        object_bindings,
                    )
                }),
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_stateful_object_binding_from_expression(
        &self,
        expression: &Expression,
        local_bindings: &HashMap<String, Expression>,
        value_bindings: &HashMap<String, Expression>,
        object_bindings: &HashMap<String, ObjectValueBinding>,
    ) -> Option<ObjectValueBinding> {
        match expression {
            Expression::Identifier(name) => object_bindings.get(name).cloned().or_else(|| {
                local_bindings
                    .get(name)
                    .or_else(|| value_bindings.get(name))
                    .filter(
                        |value| !matches!(value, Expression::Identifier(alias) if alias == name),
                    )
                    .and_then(|value| {
                        self.resolve_stateful_object_binding_from_expression(
                            value,
                            local_bindings,
                            value_bindings,
                            object_bindings,
                        )
                    })
            }),
            _ => self.infer_global_object_binding_with_state(
                expression,
                &mut value_bindings.clone(),
                &mut object_bindings.clone(),
            ),
        }
    }

    pub(in crate::backend::direct_wasm) fn infer_enumerated_keys_binding(
        &self,
        expression: &Expression,
    ) -> Option<ArrayValueBinding> {
        if let Some(array_binding) = self.infer_global_array_binding(expression) {
            return Some(enumerated_keys_from_array_binding(&array_binding));
        }
        if let Some(object_binding) = self.infer_global_object_binding(expression) {
            return Some(enumerated_keys_from_object_binding(&object_binding));
        }
        None
    }

    pub(in crate::backend::direct_wasm) fn infer_own_property_names_binding(
        &self,
        expression: &Expression,
    ) -> Option<ArrayValueBinding> {
        if let Some(array_binding) = self.infer_global_array_binding(expression) {
            return Some(own_property_names_from_array_binding(&array_binding));
        }
        let object_binding = self.infer_global_object_binding(expression);
        let has_prototype_binding = matches!(
            expression,
            Expression::Identifier(name) if self.global_prototype_object_bindings.contains_key(name)
        );
        if self.infer_global_function_binding(expression).is_some() || has_prototype_binding {
            return Some(own_property_names_from_function_binding(
                object_binding.as_ref(),
            ));
        }
        if let Some(object_binding) = object_binding {
            return Some(own_property_names_from_object_binding(&object_binding));
        }
        None
    }

    pub(in crate::backend::direct_wasm) fn infer_own_property_symbols_binding(
        &self,
        expression: &Expression,
    ) -> Option<ArrayValueBinding> {
        let object_binding = self.infer_global_object_binding(expression)?;
        Some(own_property_symbols_from_object_binding(&object_binding))
    }

    pub(in crate::backend::direct_wasm) fn infer_global_builtin_array_call_binding(
        &self,
        callee: &Expression,
        arguments: &[CallArgument],
    ) -> Option<ArrayValueBinding> {
        let Expression::Member { object, property } = callee else {
            return None;
        };
        if !matches!(object.as_ref(), Expression::Identifier(name) if name == "Object") {
            return None;
        }
        let [CallArgument::Expression(target), ..] = arguments else {
            return None;
        };
        match property.as_ref() {
            Expression::String(name) if name == "keys" => {
                self.infer_enumerated_keys_binding(target)
            }
            Expression::String(name) if name == "getOwnPropertyNames" => {
                self.infer_own_property_names_binding(target)
            }
            Expression::String(name) if name == "getOwnPropertySymbols" => {
                self.infer_own_property_symbols_binding(target)
            }
            _ => None,
        }
    }

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

    pub(in crate::backend::direct_wasm) fn infer_global_function_binding(
        &self,
        expression: &Expression,
    ) -> Option<LocalFunctionBinding> {
        match expression {
            Expression::Identifier(name) => {
                if let Some(binding) = self.global_function_bindings.get(name) {
                    return Some(binding.clone());
                }
                if is_internal_user_function_identifier(name)
                    && self.user_function_map.contains_key(name)
                {
                    Some(LocalFunctionBinding::User(name.clone()))
                } else if builtin_identifier_kind(name) == Some(StaticValueKind::Function) {
                    Some(LocalFunctionBinding::Builtin(name.clone()))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn update_static_global_assignment_metadata(
        &mut self,
        name: &str,
        value: &Expression,
    ) {
        let snapshot_value = self
            .global_value_bindings
            .get(name)
            .map(|snapshot| substitute_self_referential_binding_snapshot(value, name, snapshot))
            .unwrap_or_else(|| value.clone());
        self.global_kinds.insert(
            name.to_string(),
            infer_global_expression_kind(&snapshot_value),
        );
        self.global_value_bindings.insert(
            name.to_string(),
            self.materialize_global_expression(&snapshot_value),
        );
        if let Some(array_binding) = self.infer_global_array_binding(&snapshot_value) {
            self.global_array_bindings
                .insert(name.to_string(), array_binding);
        } else {
            self.global_array_bindings.remove(name);
        }
        if let Some(object_binding) = self.infer_global_object_binding(&snapshot_value) {
            self.global_object_bindings
                .insert(name.to_string(), object_binding);
        } else {
            self.global_object_bindings.remove(name);
        }
        if let Some(arguments_binding) = self.infer_global_arguments_binding(&snapshot_value) {
            self.global_arguments_bindings
                .insert(name.to_string(), arguments_binding);
        } else {
            self.global_arguments_bindings.remove(name);
        }
        if let Some(function_binding) = self.infer_global_function_binding(&snapshot_value) {
            self.global_function_bindings
                .insert(name.to_string(), function_binding);
            self.global_kinds
                .insert(name.to_string(), StaticValueKind::Function);
        } else {
            self.global_function_bindings.remove(name);
        }
        self.update_global_object_literal_member_bindings_for_value(name, &snapshot_value);
        self.update_global_object_literal_home_bindings(name, &snapshot_value);
        self.update_global_object_prototype_binding_from_value(name, &snapshot_value);
    }

    pub(in crate::backend::direct_wasm) fn global_member_function_binding_property(
        &self,
        property: &Expression,
    ) -> Option<MemberFunctionBindingProperty> {
        let materialized = self.materialize_global_expression(property);
        if let Some(property_name) = static_property_name_from_expression(&materialized) {
            return Some(MemberFunctionBindingProperty::String(property_name));
        }
        match &materialized {
            Expression::Member { object, property }
                if matches!(object.as_ref(), Expression::Identifier(name) if name == "Symbol")
                    && matches!(property.as_ref(), Expression::String(_)) =>
            {
                let Expression::String(symbol_name) = property.as_ref() else {
                    unreachable!("filtered above");
                };
                Some(MemberFunctionBindingProperty::Symbol(format!(
                    "Symbol.{symbol_name}"
                )))
            }
            Expression::Call { callee, .. } if matches!(callee.as_ref(), Expression::Identifier(name) if name == "Symbol") => {
                Some(MemberFunctionBindingProperty::SymbolExpression(format!(
                    "{materialized:?}"
                )))
            }
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn global_member_function_binding_key(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> Option<MemberFunctionBindingKey> {
        let target = match object {
            Expression::Identifier(name) => MemberFunctionBindingTarget::Identifier(name.clone()),
            Expression::Member {
                object,
                property: target_property,
            } if matches!(target_property.as_ref(), Expression::String(name) if name == "prototype") =>
            {
                let Expression::Identifier(name) = object.as_ref() else {
                    return None;
                };
                MemberFunctionBindingTarget::Prototype(name.clone())
            }
            _ => return None,
        };
        let property = self.global_member_function_binding_property(property)?;
        Some(MemberFunctionBindingKey { target, property })
    }

    pub(in crate::backend::direct_wasm) fn update_global_member_assignment_metadata(
        &mut self,
        object: &Expression,
        property: &Expression,
        value: &Expression,
    ) {
        let materialized_property = self.materialize_global_expression(property);
        let materialized_value = self.materialize_global_expression(value);
        match object {
            Expression::Identifier(name) if self.global_bindings.contains_key(name) => {
                let object_binding = self
                    .global_object_bindings
                    .entry(name.clone())
                    .or_insert_with(empty_object_value_binding);
                object_binding_set_property(
                    object_binding,
                    materialized_property.clone(),
                    materialized_value.clone(),
                );
            }
            Expression::Member {
                object: prototype_object,
                property: target_property,
            } if matches!(target_property.as_ref(), Expression::String(name) if name == "prototype") =>
            {
                let Expression::Identifier(name) = prototype_object.as_ref() else {
                    return;
                };
                let object_binding = self
                    .global_prototype_object_bindings
                    .entry(name.clone())
                    .or_insert_with(empty_object_value_binding);
                object_binding_set_property(
                    object_binding,
                    materialized_property.clone(),
                    materialized_value.clone(),
                );
            }
            _ => {}
        }

        let Some(key) = self.global_member_function_binding_key(object, property) else {
            return;
        };
        if let Some(binding) = self.infer_global_function_binding(value) {
            self.global_member_function_bindings
                .insert(key.clone(), binding);
        } else {
            self.global_member_function_bindings.remove(&key);
        }
        self.global_member_getter_bindings.remove(&key);
        self.global_member_setter_bindings.remove(&key);
    }
}
