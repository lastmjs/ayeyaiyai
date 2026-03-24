use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_object_binding_entries_with_state(
        &self,
        entries: &[ObjectEntry],
        local_bindings: &mut HashMap<String, Expression>,
        value_bindings: &mut HashMap<String, Expression>,
        object_bindings: &mut HashMap<String, ObjectValueBinding>,
    ) -> Option<ObjectValueBinding> {
        let mut object_binding = empty_object_value_binding();
        for entry in entries {
            match entry {
                ObjectEntry::Data { key, value } => {
                    let key = self.materialize_static_expression_with_state(
                        key,
                        local_bindings,
                        value_bindings,
                        object_bindings,
                    )?;
                    let value = self.materialize_static_expression_with_state(
                        value,
                        local_bindings,
                        value_bindings,
                        object_bindings,
                    )?;
                    object_binding_set_property(&mut object_binding, key, value);
                }
                ObjectEntry::Getter { key, .. } | ObjectEntry::Setter { key, .. } => {
                    let key = self.materialize_static_expression_with_state(
                        key,
                        local_bindings,
                        value_bindings,
                        object_bindings,
                    )?;
                    object_binding_set_property(&mut object_binding, key, Expression::Undefined);
                }
                ObjectEntry::Spread(expression) => {
                    let spread_expression = self.materialize_static_expression_with_state(
                        expression,
                        local_bindings,
                        value_bindings,
                        object_bindings,
                    )?;
                    if matches!(spread_expression, Expression::Null | Expression::Undefined)
                        || matches!(
                            &spread_expression,
                            Expression::Identifier(name)
                                if name == "undefined"
                                    && self.is_unshadowed_builtin_identifier(name)
                        )
                    {
                        continue;
                    }
                    let spread_binding = self.resolve_copy_data_properties_binding_with_state(
                        &spread_expression,
                        local_bindings,
                        value_bindings,
                        object_bindings,
                    )?;
                    merge_enumerable_object_binding(&mut object_binding, &spread_binding);
                }
            }
        }
        Some(object_binding)
    }

    pub(in crate::backend::direct_wasm) fn resolve_copy_data_properties_binding_with_state(
        &self,
        expression: &Expression,
        local_bindings: &mut HashMap<String, Expression>,
        value_bindings: &mut HashMap<String, Expression>,
        object_bindings: &mut HashMap<String, ObjectValueBinding>,
    ) -> Option<ObjectValueBinding> {
        let source_binding = self.resolve_object_binding_from_expression_with_state(
            expression,
            local_bindings,
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
                .resolve_member_getter_value_with_state(
                    expression,
                    &property,
                    local_bindings,
                    value_bindings,
                    object_bindings,
                )
                .or_else(|| {
                    self.resolve_object_binding_from_expression_with_state(
                        expression,
                        local_bindings,
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
                .resolve_member_getter_value_with_state(
                    expression,
                    property,
                    local_bindings,
                    value_bindings,
                    object_bindings,
                )
                .or_else(|| {
                    self.resolve_object_binding_from_expression_with_state(
                        expression,
                        local_bindings,
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

    pub(in crate::backend::direct_wasm) fn resolve_member_getter_value_with_state(
        &self,
        object: &Expression,
        property: &Expression,
        local_bindings: &mut HashMap<String, Expression>,
        value_bindings: &mut HashMap<String, Expression>,
        object_bindings: &mut HashMap<String, ObjectValueBinding>,
    ) -> Option<Expression> {
        let binding = self.resolve_member_getter_binding(object, property)?;
        self.execute_function_binding_with_state(
            &binding,
            local_bindings,
            value_bindings,
            object_bindings,
        )
    }

    pub(in crate::backend::direct_wasm) fn execute_function_binding_with_state(
        &self,
        binding: &LocalFunctionBinding,
        _local_bindings: &mut HashMap<String, Expression>,
        value_bindings: &mut HashMap<String, Expression>,
        object_bindings: &mut HashMap<String, ObjectValueBinding>,
    ) -> Option<Expression> {
        let LocalFunctionBinding::User(function_name) = binding else {
            return None;
        };
        let function = self.resolve_registered_function_declaration(function_name)?;
        let mut function_locals = HashMap::new();
        let result = self.execute_static_statements_with_state(
            &function.body,
            &mut function_locals,
            value_bindings,
            object_bindings,
        )?;
        Some(result.unwrap_or(Expression::Undefined))
    }

    pub(in crate::backend::direct_wasm) fn execute_static_statements_with_state(
        &self,
        statements: &[Statement],
        local_bindings: &mut HashMap<String, Expression>,
        value_bindings: &mut HashMap<String, Expression>,
        object_bindings: &mut HashMap<String, ObjectValueBinding>,
    ) -> Option<Option<Expression>> {
        for statement in statements {
            match statement {
                Statement::Block { body } => {
                    if let Some(Some(result)) = self.execute_static_statements_with_state(
                        body,
                        local_bindings,
                        value_bindings,
                        object_bindings,
                    ) {
                        return Some(Some(result));
                    }
                }
                Statement::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    let condition = self.evaluate_static_expression_with_state(
                        condition,
                        local_bindings,
                        value_bindings,
                        object_bindings,
                    )?;
                    let branch = match condition {
                        Expression::Bool(true) => then_branch,
                        Expression::Bool(false) => else_branch,
                        _ => return None,
                    };
                    if let Some(Some(result)) = self.execute_static_statements_with_state(
                        branch,
                        local_bindings,
                        value_bindings,
                        object_bindings,
                    ) {
                        return Some(Some(result));
                    }
                }
                Statement::Var { name, value } | Statement::Let { name, value, .. } => {
                    let value = self.evaluate_static_expression_with_state(
                        value,
                        local_bindings,
                        value_bindings,
                        object_bindings,
                    )?;
                    local_bindings.insert(name.clone(), value);
                }
                Statement::Assign { name, value } => {
                    let value = self.evaluate_static_expression_with_state(
                        value,
                        local_bindings,
                        value_bindings,
                        object_bindings,
                    )?;
                    self.assign_static_expression_with_state(
                        name,
                        value,
                        local_bindings,
                        value_bindings,
                        object_bindings,
                    )?;
                }
                Statement::AssignMember {
                    object,
                    property,
                    value,
                } => {
                    let property = self.evaluate_static_expression_with_state(
                        property,
                        local_bindings,
                        value_bindings,
                        object_bindings,
                    )?;
                    let value = self.evaluate_static_expression_with_state(
                        value,
                        local_bindings,
                        value_bindings,
                        object_bindings,
                    )?;
                    self.assign_static_member_expression_with_state(
                        object,
                        property,
                        value,
                        local_bindings,
                        value_bindings,
                        object_bindings,
                    )?;
                }
                Statement::Print { values } => {
                    for value in values {
                        self.evaluate_static_expression_with_state(
                            value,
                            local_bindings,
                            value_bindings,
                            object_bindings,
                        )?;
                    }
                }
                Statement::Expression(expression) => {
                    self.evaluate_static_expression_with_state(
                        expression,
                        local_bindings,
                        value_bindings,
                        object_bindings,
                    )?;
                }
                Statement::Throw(expression) => {
                    self.evaluate_static_expression_with_state(
                        expression,
                        local_bindings,
                        value_bindings,
                        object_bindings,
                    )?;
                    return Some(Some(Expression::Undefined));
                }
                Statement::Return(expression) => {
                    return Some(Some(self.evaluate_static_expression_with_state(
                        expression,
                        local_bindings,
                        value_bindings,
                        object_bindings,
                    )?));
                }
                _ => return None,
            }
        }
        Some(None)
    }

    pub(in crate::backend::direct_wasm) fn materialize_static_expression_with_state(
        &self,
        expression: &Expression,
        local_bindings: &HashMap<String, Expression>,
        value_bindings: &HashMap<String, Expression>,
        object_bindings: &HashMap<String, ObjectValueBinding>,
    ) -> Option<Expression> {
        match expression {
            Expression::Identifier(name) => {
                if self.lookup_identifier_kind(name) == Some(StaticValueKind::Symbol) {
                    return Some(Expression::Identifier(name.clone()));
                }
                if self
                    .local_value_bindings
                    .get(name)
                    .or_else(|| self.module.global_value_bindings.get(name))
                    .is_some_and(|value| {
                        matches!(
                            value,
                            Expression::Call { callee, .. }
                                if matches!(callee.as_ref(), Expression::Identifier(symbol_name)
                                    if symbol_name == "Symbol"
                                        && self.is_unshadowed_builtin_identifier(symbol_name))
                        )
                    })
                {
                    return Some(Expression::Identifier(name.clone()));
                }
                if let Some(value) = local_bindings.get(name) {
                    return self.materialize_static_expression_with_state(
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
                        return self.materialize_static_expression_with_state(
                            value,
                            local_bindings,
                            value_bindings,
                            object_bindings,
                        );
                    }
                }
                Some(self.materialize_static_expression(expression))
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
                let mut value_bindings = value_bindings.clone();
                let mut object_bindings = object_bindings.clone();
                let object_binding = self.resolve_object_binding_from_expression_with_state(
                    object,
                    local_bindings,
                    &mut value_bindings,
                    &mut object_bindings,
                )?;
                let property = self.materialize_static_expression_with_state(
                    property,
                    local_bindings,
                    &value_bindings,
                    &object_bindings,
                )?;
                if let Some(value) = object_binding_lookup_value(&object_binding, &property) {
                    return self.materialize_static_expression_with_state(
                        value,
                        local_bindings,
                        &value_bindings,
                        &object_bindings,
                    );
                }
                if static_property_name_from_expression(&property).is_some()
                    || object_binding_has_property(&object_binding, &property)
                {
                    return Some(Expression::Undefined);
                }
                None
            }
            _ => Some(self.materialize_static_expression(expression)),
        }
    }

    pub(in crate::backend::direct_wasm) fn evaluate_static_expression_with_state(
        &self,
        expression: &Expression,
        local_bindings: &mut HashMap<String, Expression>,
        value_bindings: &mut HashMap<String, Expression>,
        object_bindings: &mut HashMap<String, ObjectValueBinding>,
    ) -> Option<Expression> {
        match expression {
            Expression::Assign { name, value } => {
                let value = self.evaluate_static_expression_with_state(
                    value,
                    local_bindings,
                    value_bindings,
                    object_bindings,
                )?;
                self.assign_static_expression_with_state(
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
                let property = self.evaluate_static_expression_with_state(
                    property,
                    local_bindings,
                    value_bindings,
                    object_bindings,
                )?;
                let value = self.evaluate_static_expression_with_state(
                    value,
                    local_bindings,
                    value_bindings,
                    object_bindings,
                )?;
                self.assign_static_member_expression_with_state(
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
                    let property = self.evaluate_static_expression_with_state(
                        property,
                        local_bindings,
                        value_bindings,
                        object_bindings,
                    )?;
                    let target_name = self.resolve_stateful_object_binding_name_with_state(
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
                self.assign_static_expression_with_state(
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
                    last = self.evaluate_static_expression_with_state(
                        expression,
                        local_bindings,
                        value_bindings,
                        object_bindings,
                    )?;
                }
                Some(last)
            }
            _ => self.materialize_static_expression_with_state(
                expression,
                local_bindings,
                value_bindings,
                object_bindings,
            ),
        }
    }

    pub(in crate::backend::direct_wasm) fn assign_static_expression_with_state(
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
        if let Some(object_binding) = self.resolve_object_binding_from_expression_with_state(
            &value,
            local_bindings,
            value_bindings,
            object_bindings,
        ) {
            object_bindings.insert(name.to_string(), object_binding);
        } else {
            object_bindings.remove(name);
        }
        Some(value)
    }

    pub(in crate::backend::direct_wasm) fn assign_static_member_expression_with_state(
        &self,
        object: &Expression,
        property: Expression,
        value: Expression,
        local_bindings: &mut HashMap<String, Expression>,
        value_bindings: &mut HashMap<String, Expression>,
        object_bindings: &mut HashMap<String, ObjectValueBinding>,
    ) -> Option<Expression> {
        let target_name = self.resolve_stateful_object_binding_name_with_state(
            object,
            local_bindings,
            value_bindings,
            object_bindings,
        )?;
        let binding = object_bindings.get_mut(&target_name)?;
        object_binding_set_property(binding, property, value.clone());
        Some(value)
    }

    pub(in crate::backend::direct_wasm) fn resolve_stateful_object_binding_name_with_state(
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
                    self.resolve_stateful_object_binding_name_with_state(
                        value,
                        local_bindings,
                        value_bindings,
                        object_bindings,
                    )
                }),
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_object_binding_from_expression_with_state(
        &self,
        expression: &Expression,
        local_bindings: &HashMap<String, Expression>,
        value_bindings: &mut HashMap<String, Expression>,
        object_bindings: &mut HashMap<String, ObjectValueBinding>,
    ) -> Option<ObjectValueBinding> {
        match expression {
            Expression::Identifier(name) => {
                if let Some(binding) = object_bindings.get(name).cloned() {
                    return Some(binding);
                }
                let next = local_bindings
                    .get(name)
                    .cloned()
                    .or_else(|| value_bindings.get(name).cloned())
                    .filter(
                        |value| !matches!(value, Expression::Identifier(alias) if alias == name),
                    );
                next.and_then(|value| {
                    self.resolve_object_binding_from_expression_with_state(
                        &value,
                        local_bindings,
                        value_bindings,
                        object_bindings,
                    )
                })
            }
            Expression::Object(entries) => self.resolve_object_binding_entries_with_state(
                entries,
                &mut local_bindings.clone(),
                value_bindings,
                object_bindings,
            ),
            _ => self.resolve_object_binding_from_expression(expression),
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_enumerated_keys_binding(
        &self,
        expression: &Expression,
    ) -> Option<ArrayValueBinding> {
        if let Some(array_binding) = self.resolve_array_binding_from_expression(expression) {
            return Some(enumerated_keys_from_array_binding(&array_binding));
        }
        if let Some(object_binding) = self.resolve_object_binding_from_expression(expression) {
            return Some(enumerated_keys_from_object_binding(&object_binding));
        }
        None
    }

    pub(in crate::backend::direct_wasm) fn resolve_own_property_names_binding(
        &self,
        expression: &Expression,
    ) -> Option<ArrayValueBinding> {
        if let Some(array_binding) = self.resolve_array_binding_from_expression(expression) {
            return Some(own_property_names_from_array_binding(&array_binding));
        }
        let object_binding = self.resolve_object_binding_from_expression(expression);
        let has_prototype_binding = matches!(
            expression,
            Expression::Identifier(name)
                if self.local_prototype_object_bindings.contains_key(name)
                    || self.module.global_prototype_object_bindings.contains_key(name)
        );
        if self
            .resolve_function_binding_from_expression(expression)
            .is_some()
            || has_prototype_binding
        {
            return Some(own_property_names_from_function_binding(
                object_binding.as_ref(),
            ));
        }
        if let Some(object_binding) = object_binding {
            return Some(own_property_names_from_object_binding(&object_binding));
        }
        None
    }

    pub(in crate::backend::direct_wasm) fn resolve_own_property_symbols_binding(
        &self,
        expression: &Expression,
    ) -> Option<ArrayValueBinding> {
        let object_binding = self.resolve_object_binding_from_expression(expression)?;
        Some(own_property_symbols_from_object_binding(&object_binding))
    }

    pub(in crate::backend::direct_wasm) fn resolve_builtin_array_call_binding(
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
                self.resolve_enumerated_keys_binding(target)
            }
            Expression::String(name) if name == "getOwnPropertyNames" => {
                self.resolve_own_property_names_binding(target)
            }
            Expression::String(name) if name == "getOwnPropertySymbols" => {
                self.resolve_own_property_symbols_binding(target)
            }
            _ => None,
        }
    }
}
