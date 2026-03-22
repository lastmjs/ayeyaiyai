use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_object_binding_from_expression(
        &self,
        expression: &Expression,
    ) -> Option<ObjectValueBinding> {
        let binding = match expression {
            Expression::Identifier(name) => {
                if name == "$262" {
                    let mut host_object_binding = empty_object_value_binding();
                    object_binding_set_property(
                        &mut host_object_binding,
                        Expression::String("createRealm".to_string()),
                        Expression::Identifier(TEST262_CREATE_REALM_BUILTIN.to_string()),
                    );
                    return Some(host_object_binding);
                }
                if let Some(realm_id) = parse_test262_realm_identifier(name) {
                    return self.module.test262_realm_object_binding(realm_id);
                }
                if let Some(realm_id) = parse_test262_realm_global_identifier(name) {
                    return self
                        .module
                        .test262_realms
                        .get(&realm_id)
                        .map(|realm| realm.global_object_binding.clone());
                }
                self.local_object_bindings
                    .get(name)
                    .cloned()
                    .or_else(|| {
                        let hidden_name = self.resolve_user_function_capture_hidden_name(name)?;
                        self.module
                            .global_object_bindings
                            .get(&hidden_name)
                            .cloned()
                    })
                    .or_else(|| self.module.global_object_bindings.get(name).cloned())
                    .or_else(|| {
                        let proxy = self
                            .local_proxy_bindings
                            .get(name)
                            .cloned()
                            .or_else(|| self.module.global_proxy_bindings.get(name).cloned())?;
                        self.resolve_object_binding_from_expression(&proxy.target)
                    })
                    .or_else(|| {
                        let resolved = self.resolve_bound_alias_expression(expression)?;
                        (!static_expression_matches(&resolved, expression))
                            .then(|| self.resolve_object_binding_from_expression(&resolved))
                            .flatten()
                    })
            }
            Expression::Member { object, property } if matches!(property.as_ref(), Expression::String(name) if name == "global") =>
            {
                let realm_id = self.resolve_test262_realm_id_from_expression(object)?;
                self.module
                    .test262_realms
                    .get(&realm_id)
                    .map(|realm| realm.global_object_binding.clone())
            }
            Expression::Member { object, property } if matches!(property.as_ref(), Expression::String(name) if name == "prototype") =>
            {
                let Expression::Identifier(name) = object.as_ref() else {
                    return None;
                };
                self.local_prototype_object_bindings
                    .get(name)
                    .cloned()
                    .or_else(|| {
                        self.module
                            .global_prototype_object_bindings
                            .get(name)
                            .cloned()
                    })
            }
            Expression::GetIterator(iterated) => {
                let iterator_callee = Expression::Member {
                    object: Box::new((**iterated).clone()),
                    property: Box::new(
                        self.materialize_static_expression(&symbol_iterator_expression()),
                    ),
                };
                self.resolve_object_binding_from_expression(&Expression::Call {
                    callee: Box::new(iterator_callee),
                    arguments: Vec::new(),
                })
            }
            Expression::Call { callee, .. }
                if matches!(
                    callee.as_ref(),
                    Expression::Member { object, property }
                        if matches!(object.as_ref(), Expression::Identifier(name) if name == "Object")
                            && matches!(property.as_ref(), Expression::String(name) if name == "create")
                ) =>
            {
                Some(ObjectValueBinding {
                    string_properties: Vec::new(),
                    symbol_properties: Vec::new(),
                    non_enumerable_string_properties: Vec::new(),
                })
            }
            Expression::Call { callee, arguments }
                if arguments.is_empty()
                    && matches!(
                        callee.as_ref(),
                        Expression::Member { object, property }
                            if matches!(object.as_ref(), Expression::Identifier(name) if name == "$262")
                                && matches!(property.as_ref(), Expression::String(name) if name == "createRealm")
                    ) =>
            {
                Some(empty_object_value_binding())
            }
            Expression::New { callee, arguments }
                if arguments.is_empty()
                    && matches!(callee.as_ref(), Expression::Identifier(name) if name == "Object") =>
            {
                Some(empty_object_value_binding())
            }
            Expression::Call { callee, arguments } => self
                .resolve_static_call_result_expression_with_context(
                    callee,
                    arguments,
                    self.current_user_function_name.as_deref(),
                )
                .filter(|_| matches!(callee.as_ref(), Expression::Identifier(_)))
                .and_then(|(result_expression, _)| {
                    self.resolve_object_binding_from_expression(&result_expression)
                })
                .or_else(|| self.resolve_returned_object_binding_from_call(callee, arguments))
                .or_else(|| {
                    if !arguments.is_empty() {
                        return None;
                    }
                    let LocalFunctionBinding::User(function_name) = self
                        .resolve_function_binding_from_expression_with_context(
                            callee,
                            self.current_user_function_name.as_deref(),
                        )?
                    else {
                        return None;
                    };
                    let (result_expression, _) = self
                        .execute_simple_static_user_function_with_bindings(
                            &function_name,
                            &HashMap::new(),
                        )?;
                    self.resolve_object_binding_from_expression(&result_expression)
                }),
            Expression::Object(entries) => {
                let mut value_bindings = self.module.global_value_bindings.clone();
                value_bindings.extend(self.local_value_bindings.clone());
                let mut object_bindings = self.module.global_object_bindings.clone();
                object_bindings.extend(self.local_object_bindings.clone());
                self.resolve_object_binding_entries_with_state(
                    entries,
                    &mut HashMap::new(),
                    &mut value_bindings,
                    &mut object_bindings,
                )
            }
            _ => self
                .resolve_bound_alias_expression(expression)
                .filter(|resolved| resolved != expression)
                .and_then(|resolved| self.resolve_object_binding_from_expression(&resolved)),
        };
        if binding.is_some() {
            return binding;
        }

        let materialized = self.materialize_static_expression(expression);
        if !static_expression_matches(&materialized, expression) {
            return self.resolve_object_binding_from_expression(&materialized);
        }
        None
    }

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
        for statement in &function.body {
            match statement {
                Statement::Var { name, value } | Statement::Let { name, value, .. } => {
                    let value = self.evaluate_static_expression_with_state(
                        value,
                        &mut function_locals,
                        value_bindings,
                        object_bindings,
                    )?;
                    function_locals.insert(name.clone(), value);
                }
                Statement::Assign { name, value } => {
                    let value = self.evaluate_static_expression_with_state(
                        value,
                        &mut function_locals,
                        value_bindings,
                        object_bindings,
                    )?;
                    self.assign_static_expression_with_state(
                        name,
                        value,
                        &mut function_locals,
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
                        &mut function_locals,
                        value_bindings,
                        object_bindings,
                    )?;
                    let value = self.evaluate_static_expression_with_state(
                        value,
                        &mut function_locals,
                        value_bindings,
                        object_bindings,
                    )?;
                    self.assign_static_member_expression_with_state(
                        object,
                        property,
                        value,
                        &mut function_locals,
                        value_bindings,
                        object_bindings,
                    )?;
                }
                Statement::Expression(expression) => {
                    self.evaluate_static_expression_with_state(
                        expression,
                        &mut function_locals,
                        value_bindings,
                        object_bindings,
                    )?;
                }
                Statement::Return(expression) => {
                    return self.evaluate_static_expression_with_state(
                        expression,
                        &mut function_locals,
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

    pub(in crate::backend::direct_wasm) fn materialize_static_expression(
        &self,
        expression: &Expression,
    ) -> Expression {
        let guard_key = expression as *const Expression as usize;
        {
            let mut active = self.materializing_expression_keys.borrow_mut();
            if !active.insert(guard_key) {
                return expression.clone();
            }
        }
        let _guard = MaterializationGuard {
            active: &self.materializing_expression_keys,
            key: guard_key,
        };
        match expression {
            Expression::Identifier(name) => {
                if self.local_object_bindings.contains_key(name)
                    || self.module.global_object_bindings.contains_key(name)
                    || self.local_prototype_object_bindings.contains_key(name)
                    || self
                        .module
                        .global_prototype_object_bindings
                        .contains_key(name)
                {
                    return expression.clone();
                }
                if self.local_array_bindings.contains_key(name)
                    || self.module.global_array_bindings.contains_key(name)
                    || self.local_typed_array_view_bindings.contains_key(name)
                {
                    return expression.clone();
                }
                if let Some(symbol_identity) = self.resolve_symbol_identity_expression(expression) {
                    return symbol_identity;
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
                    return Expression::Identifier(name.clone());
                }
                if let Some(resolved) = self.resolve_bound_alias_expression(expression) {
                    if !static_expression_matches(&resolved, expression) {
                        return self.materialize_static_expression(&resolved);
                    }
                }
                expression.clone()
            }
            Expression::Member { object, property } => {
                if let Some(step_binding) =
                    self.resolve_iterator_step_binding_from_expression(object)
                    && let Expression::String(property_name) = property.as_ref()
                {
                    match (property_name.as_str(), step_binding) {
                        (
                            "done",
                            IteratorStepBinding::Runtime {
                                static_done: Some(done),
                                ..
                            },
                        ) => return Expression::Bool(done),
                        (
                            "value",
                            IteratorStepBinding::Runtime {
                                static_value: Some(value),
                                ..
                            },
                        ) => return self.materialize_static_expression(&value),
                        _ => {}
                    }
                }
                if let Some(array_binding) = self.resolve_array_binding_from_expression(object) {
                    if matches!(property.as_ref(), Expression::String(text) if text == "length") {
                        if self
                            .runtime_array_length_local_for_expression(object)
                            .is_some()
                        {
                            return Expression::Member {
                                object: Box::new(self.materialize_static_expression(object)),
                                property: Box::new(self.materialize_static_expression(property)),
                            };
                        }
                        return Expression::Number(array_binding.values.len() as f64);
                    }
                    if let Some(index) = argument_index_from_expression(property) {
                        if let Some(Some(value)) = array_binding.values.get(index as usize) {
                            return self.materialize_static_expression(value);
                        }
                        return Expression::Undefined;
                    }
                }
                if let Some(object_binding) = self.resolve_object_binding_from_expression(object) {
                    let materialized_property = self.materialize_static_expression(property);
                    if let Some(value) =
                        object_binding_lookup_value(&object_binding, &materialized_property)
                    {
                        return self.materialize_static_expression(value);
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
                    object: Box::new(self.materialize_static_expression(object)),
                    property: Box::new(self.materialize_static_expression(property)),
                }
            }
            Expression::Unary { op, expression } => Expression::Unary {
                op: *op,
                expression: Box::new(self.materialize_static_expression(expression)),
            },
            Expression::Binary { op, left, right } => Expression::Binary {
                op: *op,
                left: Box::new(self.materialize_static_expression(left)),
                right: Box::new(self.materialize_static_expression(right)),
            },
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                let materialized_condition = self.materialize_static_expression(condition);
                if let Some(condition_value) =
                    self.resolve_static_if_condition_value(&materialized_condition)
                {
                    let branch = if condition_value {
                        then_expression.as_ref()
                    } else {
                        else_expression.as_ref()
                    };
                    return self.materialize_static_expression(branch);
                }
                Expression::Conditional {
                    condition: Box::new(materialized_condition),
                    then_expression: Box::new(self.materialize_static_expression(then_expression)),
                    else_expression: Box::new(self.materialize_static_expression(else_expression)),
                }
            }
            Expression::Sequence(expressions) => Expression::Sequence(
                expressions
                    .iter()
                    .map(|expression| self.materialize_static_expression(expression))
                    .collect(),
            ),
            Expression::Array(elements) => Expression::Array(
                elements
                    .iter()
                    .map(|element| match element {
                        crate::ir::hir::ArrayElement::Expression(expression) => {
                            crate::ir::hir::ArrayElement::Expression(
                                self.materialize_static_expression(expression),
                            )
                        }
                        crate::ir::hir::ArrayElement::Spread(expression) => {
                            crate::ir::hir::ArrayElement::Spread(
                                self.materialize_static_expression(expression),
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
                                key: self.materialize_static_expression(key),
                                value: self.materialize_static_expression(value),
                            }
                        }
                        crate::ir::hir::ObjectEntry::Getter { key, getter } => {
                            crate::ir::hir::ObjectEntry::Getter {
                                key: self.materialize_static_expression(key),
                                getter: self.materialize_static_expression(getter),
                            }
                        }
                        crate::ir::hir::ObjectEntry::Setter { key, setter } => {
                            crate::ir::hir::ObjectEntry::Setter {
                                key: self.materialize_static_expression(key),
                                setter: self.materialize_static_expression(setter),
                            }
                        }
                        crate::ir::hir::ObjectEntry::Spread(expression) => {
                            crate::ir::hir::ObjectEntry::Spread(
                                self.materialize_static_expression(expression),
                            )
                        }
                    })
                    .collect(),
            ),
            Expression::Call { callee, arguments } => {
                if arguments.is_empty()
                    && let Expression::Member { object, property } = callee.as_ref()
                    && let Expression::String(property_name) = property.as_ref()
                    && matches!(property_name.as_str(), "toString" | "valueOf")
                    && let Some(StaticEvalOutcome::Value(value)) = self
                        .resolve_static_member_call_outcome_with_context(
                            object,
                            property_name,
                            self.current_user_function_name.as_deref(),
                        )
                {
                    return self.materialize_static_expression(&value);
                }
                if matches!(callee.as_ref(), Expression::Identifier(_))
                    && let Some(value) =
                        self.resolve_static_call_result_expression(callee, arguments)
                {
                    return self.materialize_static_expression(&value);
                }
                Expression::Call {
                    callee: Box::new(self.materialize_static_expression(callee)),
                    arguments: arguments
                        .iter()
                        .map(|argument| match argument {
                            CallArgument::Expression(expression) => CallArgument::Expression(
                                self.materialize_static_expression(expression),
                            ),
                            CallArgument::Spread(expression) => {
                                CallArgument::Spread(self.materialize_static_expression(expression))
                            }
                        })
                        .collect(),
                }
            }
            Expression::Assign { name, value } => Expression::Assign {
                name: name.clone(),
                value: Box::new(self.materialize_static_expression(value)),
            },
            Expression::AssignMember {
                object,
                property,
                value,
            } => Expression::AssignMember {
                object: Box::new(self.materialize_static_expression(object)),
                property: Box::new(self.materialize_static_expression(property)),
                value: Box::new(self.materialize_static_expression(value)),
            },
            Expression::AssignSuperMember { property, value } => Expression::AssignSuperMember {
                property: Box::new(self.materialize_static_expression(property)),
                value: Box::new(self.materialize_static_expression(value)),
            },
            Expression::Await(value) => {
                Expression::Await(Box::new(self.materialize_static_expression(value)))
            }
            Expression::EnumerateKeys(value) => {
                Expression::EnumerateKeys(Box::new(self.materialize_static_expression(value)))
            }
            Expression::GetIterator(value) => {
                Expression::GetIterator(Box::new(self.materialize_static_expression(value)))
            }
            Expression::IteratorClose(value) => {
                Expression::IteratorClose(Box::new(self.materialize_static_expression(value)))
            }
            Expression::New { callee, arguments } => Expression::New {
                callee: Box::new(self.materialize_static_expression(callee)),
                arguments: arguments
                    .iter()
                    .map(|argument| match argument {
                        CallArgument::Expression(expression) => {
                            CallArgument::Expression(self.materialize_static_expression(expression))
                        }
                        CallArgument::Spread(expression) => {
                            CallArgument::Spread(self.materialize_static_expression(expression))
                        }
                    })
                    .collect(),
            },
            _ => expression.clone(),
        }
    }

    pub(in crate::backend::direct_wasm) fn update_local_object_binding(
        &mut self,
        name: &str,
        value: &Expression,
    ) {
        let Some(object_binding) = self.resolve_object_binding_from_expression(value) else {
            self.local_object_bindings.remove(name);
            if self.binding_name_is_global(name) {
                self.module.global_object_bindings.remove(name);
            }
            return;
        };
        self.local_object_bindings
            .insert(name.to_string(), object_binding.clone());
        if self.binding_name_is_global(name) {
            self.module
                .global_object_bindings
                .insert(name.to_string(), object_binding);
        }
        self.local_kinds
            .insert(name.to_string(), StaticValueKind::Object);
    }

    pub(in crate::backend::direct_wasm) fn update_prototype_object_binding(
        &mut self,
        name: &str,
        value: &Expression,
    ) {
        let Some(object_binding) = self.resolve_object_binding_from_expression(value) else {
            self.local_prototype_object_bindings.remove(name);
            if self.binding_name_is_global(name) {
                self.module.global_prototype_object_bindings.remove(name);
            }
            return;
        };
        self.local_prototype_object_bindings
            .insert(name.to_string(), object_binding.clone());
        if self.binding_name_is_global(name) {
            self.module
                .global_prototype_object_bindings
                .insert(name.to_string(), object_binding);
        }
    }

    pub(in crate::backend::direct_wasm) fn update_object_binding_from_expression(
        &mut self,
        expression: &Expression,
    ) {
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
            CallArgument::Expression(descriptor_expression),
            ..,
        ] = arguments.as_slice()
        else {
            return;
        };
        let Some(descriptor) = resolve_property_descriptor_definition(descriptor_expression) else {
            return;
        };

        let update_global_property_descriptor =
            |compiler: &mut Self,
             property: &Expression,
             descriptor: &PropertyDescriptorDefinition| {
                let property = compiler
                    .resolve_property_key_expression(property)
                    .unwrap_or_else(|| compiler.materialize_static_expression(property));
                let Some(property_name) = static_property_name_from_expression(&property) else {
                    return;
                };
                let existing = compiler
                    .module
                    .global_property_descriptors
                    .get(&property_name)
                    .cloned();
                let value = if descriptor.is_accessor() {
                    Expression::Undefined
                } else {
                    descriptor
                        .value
                        .as_ref()
                        .map(|expression| compiler.materialize_static_expression(expression))
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
                compiler.module.global_property_descriptors.insert(
                    property_name,
                    GlobalPropertyDescriptorState {
                        value,
                        writable,
                        enumerable,
                        configurable,
                    },
                );
            };

        match target {
            Expression::This => {
                update_global_property_descriptor(self, property, &descriptor);
            }
            Expression::Identifier(name) => {
                let property = self
                    .resolve_property_key_expression(property)
                    .unwrap_or_else(|| self.materialize_static_expression(property));
                let property_name = static_property_name_from_expression(&property);
                let existing_value = self
                    .local_object_bindings
                    .get(name)
                    .or_else(|| self.module.global_object_bindings.get(name))
                    .and_then(|object_binding| {
                        object_binding_lookup_value(object_binding, &property)
                    })
                    .cloned();
                let current_enumerable = property_name.as_ref().is_some_and(|property_name| {
                    self.local_object_bindings
                        .get(name)
                        .or_else(|| self.module.global_object_bindings.get(name))
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
                if let Some(object_binding) = self.local_object_bindings.get_mut(name) {
                    object_binding_define_property(
                        object_binding,
                        property.clone(),
                        value.clone(),
                        enumerable,
                    );
                } else if self.binding_name_is_global(name) {
                    let object_binding = self
                        .module
                        .global_object_bindings
                        .entry(name.to_string())
                        .or_insert_with(|| ObjectValueBinding {
                            string_properties: Vec::new(),
                            symbol_properties: Vec::new(),
                            non_enumerable_string_properties: Vec::new(),
                        });
                    object_binding_define_property(object_binding, property, value, enumerable);
                }
            }
            Expression::Member {
                object,
                property: target_property,
            } if matches!(target_property.as_ref(), Expression::String(name) if name == "prototype") =>
            {
                let Expression::Identifier(name) = object.as_ref() else {
                    return;
                };
                let property = self
                    .resolve_property_key_expression(property)
                    .unwrap_or_else(|| self.materialize_static_expression(property));
                let property_name = static_property_name_from_expression(&property);
                let existing_value = self
                    .local_prototype_object_bindings
                    .get(name)
                    .or_else(|| self.module.global_prototype_object_bindings.get(name))
                    .and_then(|object_binding| {
                        object_binding_lookup_value(object_binding, &property)
                    })
                    .cloned();
                let current_enumerable = property_name.as_ref().is_some_and(|property_name| {
                    self.local_prototype_object_bindings
                        .get(name)
                        .or_else(|| self.module.global_prototype_object_bindings.get(name))
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
                if let Some(object_binding) = self.local_prototype_object_bindings.get_mut(name) {
                    object_binding_define_property(
                        object_binding,
                        property.clone(),
                        value.clone(),
                        enumerable,
                    );
                }
                if self.binding_name_is_global(name) {
                    let object_binding = self
                        .module
                        .global_prototype_object_bindings
                        .entry(name.to_string())
                        .or_insert_with(|| ObjectValueBinding {
                            string_properties: Vec::new(),
                            symbol_properties: Vec::new(),
                            non_enumerable_string_properties: Vec::new(),
                        });
                    object_binding_define_property(object_binding, property, value, enumerable);
                }
            }
            _ => {}
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_static_call_result_expression(
        &self,
        callee: &Expression,
        arguments: &[CallArgument],
    ) -> Option<Expression> {
        self.resolve_static_call_result_expression_with_context(
            callee,
            arguments,
            self.current_user_function_name.as_deref(),
        )
        .map(|(value, _)| value)
    }

    pub(in crate::backend::direct_wasm) fn resolve_static_call_result_expression_with_context(
        &self,
        callee: &Expression,
        arguments: &[CallArgument],
        current_function_name: Option<&str>,
    ) -> Option<(Expression, Option<String>)> {
        let (function_name, user_function) = match self
            .resolve_function_binding_from_expression_with_context(callee, current_function_name)?
        {
            LocalFunctionBinding::User(function_name) => {
                let user_function = self.module.user_function_map.get(&function_name)?;
                (function_name, user_function)
            }
            LocalFunctionBinding::Builtin(_) => return None,
        };

        let summary = user_function.inline_summary.as_ref()?;
        if !summary.effects.is_empty() {
            return None;
        }
        let return_value = summary.return_value.as_ref()?;
        Some((
            self.substitute_user_function_argument_bindings(return_value, user_function, arguments),
            Some(function_name),
        ))
    }

    pub(in crate::backend::direct_wasm) fn update_local_arguments_binding(
        &mut self,
        name: &str,
        value: &Expression,
    ) {
        if self.is_direct_arguments_object(value) {
            self.direct_arguments_aliases.insert(name.to_string());
            self.local_arguments_bindings.remove(name);
            self.local_kinds
                .insert(name.to_string(), StaticValueKind::Object);
            return;
        }
        self.direct_arguments_aliases.remove(name);
        let Some(arguments_binding) = self.resolve_arguments_binding_from_expression(value) else {
            self.local_arguments_bindings.remove(name);
            return;
        };
        self.local_arguments_bindings
            .insert(name.to_string(), arguments_binding);
        self.local_kinds
            .insert(name.to_string(), StaticValueKind::Object);
    }

    pub(in crate::backend::direct_wasm) fn resolve_descriptor_binding_from_expression(
        &self,
        expression: &Expression,
    ) -> Option<PropertyDescriptorBinding> {
        match expression {
            Expression::Identifier(name) => self.local_descriptor_bindings.get(name).cloned(),
            Expression::Call { callee, arguments } => {
                let Expression::Member { object, property } = callee.as_ref() else {
                    return None;
                };
                if !matches!(object.as_ref(), Expression::Identifier(name) if name == "Object") {
                    return None;
                }
                if !matches!(property.as_ref(), Expression::String(name) if name == "getOwnPropertyDescriptor")
                {
                    return None;
                }
                let [
                    CallArgument::Expression(target),
                    CallArgument::Expression(property_name),
                    ..,
                ] = arguments.as_slice()
                else {
                    return None;
                };
                let property_name = match property_name {
                    Expression::String(text) => text.as_str(),
                    _ => return None,
                };
                if property_name == "length" {
                    if self.is_direct_arguments_object(target) {
                        if !self.current_arguments_length_present {
                            return None;
                        }
                    } else if !self
                        .resolve_arguments_binding_from_expression(target)?
                        .length_present
                    {
                        return None;
                    }
                    return Some(PropertyDescriptorBinding {
                        value: if self.is_direct_arguments_object(target) {
                            self.current_arguments_length_override
                                .clone()
                                .or(Some(Expression::Undefined))
                        } else {
                            Some(
                                self.resolve_arguments_binding_from_expression(target)?
                                    .length_value
                                    .clone(),
                            )
                        },
                        configurable: true,
                        enumerable: false,
                        writable: Some(true),
                        has_get: false,
                        has_set: false,
                    });
                }
                if let Ok(index) = property_name.parse::<usize>() {
                    return Some(PropertyDescriptorBinding {
                        value: if self.is_direct_arguments_object(target) {
                            self.arguments_slots
                                .get(&(index as u32))
                                .filter(|slot| slot.state.present)
                                .map(|_| Expression::Undefined)
                                .or(Some(Expression::Undefined))
                        } else {
                            Some(
                                self.resolve_arguments_binding_from_expression(target)?
                                    .values
                                    .get(index)
                                    .cloned()
                                    .unwrap_or(Expression::Undefined),
                            )
                        },
                        configurable: true,
                        enumerable: true,
                        writable: Some(true),
                        has_get: false,
                        has_set: false,
                    });
                }
                if property_name == "callee" {
                    let strict = if self.is_direct_arguments_object(target) {
                        if !self.current_arguments_callee_present {
                            return None;
                        }
                        self.strict_mode
                    } else {
                        let binding = self.resolve_arguments_binding_from_expression(target)?;
                        if !binding.callee_present {
                            return None;
                        }
                        binding.strict
                    };
                    return Some(if strict {
                        PropertyDescriptorBinding {
                            value: None,
                            configurable: false,
                            enumerable: false,
                            writable: None,
                            has_get: true,
                            has_set: true,
                        }
                    } else {
                        PropertyDescriptorBinding {
                            value: if self.is_direct_arguments_object(target) {
                                self.direct_arguments_callee_expression()
                            } else {
                                self.resolve_arguments_binding_from_expression(target)?
                                    .callee_value
                                    .clone()
                            },
                            configurable: true,
                            enumerable: false,
                            writable: Some(true),
                            has_get: false,
                            has_set: false,
                        }
                    });
                }
                if self.top_level_function && matches!(target, Expression::This) {
                    let state = self.module.global_property_descriptors.get(property_name)?;
                    return Some(PropertyDescriptorBinding {
                        value: state.writable.map(|_| state.value.clone()),
                        configurable: state.configurable,
                        enumerable: state.enumerable,
                        writable: state.writable,
                        has_get: false,
                        has_set: false,
                    });
                }
                None
            }
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn update_local_descriptor_binding(
        &mut self,
        name: &str,
        value: &Expression,
    ) {
        let Some(descriptor_binding) = self.resolve_descriptor_binding_from_expression(value)
        else {
            self.local_descriptor_bindings.remove(name);
            return;
        };
        self.local_descriptor_bindings
            .insert(name.to_string(), descriptor_binding);
        self.local_kinds
            .insert(name.to_string(), StaticValueKind::Object);
    }

    pub(in crate::backend::direct_wasm) fn update_global_property_descriptor_value(
        &mut self,
        name: &str,
        value_expression: &Expression,
    ) {
        let materialized = self
            .module
            .global_value_bindings
            .get(name)
            .cloned()
            .unwrap_or_else(|| self.materialize_static_expression(value_expression));
        if let Some(state) = self.module.global_property_descriptors.get_mut(name) {
            state.value = materialized;
        }
    }

    pub(in crate::backend::direct_wasm) fn ensure_global_property_descriptor_value(
        &mut self,
        name: &str,
        value_expression: &Expression,
        configurable: bool,
    ) {
        let materialized = self
            .module
            .global_value_bindings
            .get(name)
            .cloned()
            .unwrap_or_else(|| self.materialize_static_expression(value_expression));
        match self.module.global_property_descriptors.get_mut(name) {
            Some(state) => state.value = materialized,
            None => {
                self.module.global_property_descriptors.insert(
                    name.to_string(),
                    GlobalPropertyDescriptorState {
                        value: materialized,
                        writable: Some(true),
                        enumerable: true,
                        configurable,
                    },
                );
            }
        }
    }

    pub(in crate::backend::direct_wasm) fn instantiate_eval_global_function_property_descriptor(
        &mut self,
        name: &str,
    ) {
        let value = Expression::Identifier(name.to_string());
        match self.module.global_property_descriptors.get_mut(name) {
            Some(state) if !state.configurable => {
                state.value = value;
            }
            Some(state) => {
                *state = GlobalPropertyDescriptorState {
                    value,
                    writable: Some(true),
                    enumerable: true,
                    configurable: true,
                };
            }
            None => {
                self.module.global_property_descriptors.insert(
                    name.to_string(),
                    GlobalPropertyDescriptorState {
                        value,
                        writable: Some(true),
                        enumerable: true,
                        configurable: true,
                    },
                );
            }
        }
    }

    pub(in crate::backend::direct_wasm) fn update_local_value_binding(
        &mut self,
        name: &str,
        value: &Expression,
    ) {
        let snapshot_value = self
            .local_value_bindings
            .get(name)
            .or_else(|| self.module.global_value_bindings.get(name))
            .map(|snapshot| substitute_self_referential_binding_snapshot(value, name, snapshot))
            .unwrap_or_else(|| value.clone());
        let mut referenced_names = HashSet::new();
        collect_referenced_binding_names_from_expression(&snapshot_value, &mut referenced_names);
        if referenced_names.contains(name) {
            self.local_value_bindings.remove(name);
            return;
        }
        let materialized_value = self
            .resolve_static_string_value(&snapshot_value)
            .map(Expression::String)
            .unwrap_or_else(|| self.materialize_static_expression(&snapshot_value));
        self.local_value_bindings
            .insert(name.to_string(), materialized_value);
    }

    pub(in crate::backend::direct_wasm) fn resolve_bound_alias_expression(
        &self,
        expression: &Expression,
    ) -> Option<Expression> {
        let mut current = expression;
        let mut visited = HashSet::new();
        loop {
            let Expression::Identifier(name) = current else {
                return Some(current.clone());
            };
            if !visited.insert(name.clone()) {
                return None;
            }
            if let Some((resolved_name, _)) = self.resolve_current_local_binding(name)
                && let Some(value) = self.local_value_bindings.get(&resolved_name)
            {
                current = value;
                continue;
            }
            if let Some(value) = self.local_value_bindings.get(name) {
                current = value;
                continue;
            }
            if let Some(hidden_name) = self.resolve_user_function_capture_hidden_name(name)
                && let Some(value) = self.module.global_value_bindings.get(&hidden_name)
            {
                current = value;
                continue;
            }
            if let Some(value) = self.module.global_value_bindings.get(name) {
                current = value;
                continue;
            }
            return Some(current.clone());
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_symbol_identity_expression(
        &self,
        expression: &Expression,
    ) -> Option<Expression> {
        let Expression::Identifier(name) = expression else {
            return None;
        };
        if self.lookup_identifier_kind(name) != Some(StaticValueKind::Symbol) {
            if let Some(resolved) = self.resolve_bound_alias_expression(expression)
                && !static_expression_matches(&resolved, expression)
            {
                if self.well_known_symbol_name(&resolved).is_some() {
                    return Some(resolved);
                }
                if let Expression::Identifier(resolved_name) = &resolved
                    && self.lookup_identifier_kind(resolved_name) == Some(StaticValueKind::Symbol)
                {
                    return Some(resolved);
                }
            }
            return None;
        }

        let mut current_name = name.clone();
        let mut visited = HashSet::new();
        loop {
            if !visited.insert(current_name.clone()) {
                return None;
            }
            let next = self
                .local_value_bindings
                .get(&current_name)
                .or_else(|| self.module.global_value_bindings.get(&current_name));
            match next {
                Some(Expression::Identifier(next_name))
                    if self.lookup_identifier_kind(next_name) == Some(StaticValueKind::Symbol) =>
                {
                    current_name = next_name.clone();
                }
                _ => return Some(Expression::Identifier(current_name)),
            }
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_global_value_expression(
        &self,
        expression: &Expression,
    ) -> Option<Expression> {
        let mut visited = HashSet::new();
        self.resolve_global_value_expression_with_visited(expression, &mut visited)
    }

    pub(in crate::backend::direct_wasm) fn resolve_global_value_expression_with_visited(
        &self,
        expression: &Expression,
        visited: &mut HashSet<String>,
    ) -> Option<Expression> {
        let Expression::Identifier(name) = expression else {
            return Some(expression.clone());
        };
        if !visited.insert(name.clone()) {
            return None;
        }
        let value = self.module.global_value_bindings.get(name)?.clone();
        self.resolve_global_identifiers_in_expression(&value, visited)
    }

    pub(in crate::backend::direct_wasm) fn resolve_global_identifiers_in_expression(
        &self,
        expression: &Expression,
        visited: &mut HashSet<String>,
    ) -> Option<Expression> {
        match expression {
            Expression::Identifier(name)
                if self.module.global_value_bindings.contains_key(name) =>
            {
                self.resolve_global_value_expression_with_visited(expression, visited)
            }
            Expression::Unary { op, expression } => Some(Expression::Unary {
                op: *op,
                expression: Box::new(
                    self.resolve_global_identifiers_in_expression(expression, visited)?,
                ),
            }),
            Expression::Binary { op, left, right } => Some(Expression::Binary {
                op: *op,
                left: Box::new(self.resolve_global_identifiers_in_expression(left, visited)?),
                right: Box::new(self.resolve_global_identifiers_in_expression(right, visited)?),
            }),
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => Some(Expression::Conditional {
                condition: Box::new(
                    self.resolve_global_identifiers_in_expression(condition, visited)?,
                ),
                then_expression: Box::new(
                    self.resolve_global_identifiers_in_expression(then_expression, visited)?,
                ),
                else_expression: Box::new(
                    self.resolve_global_identifiers_in_expression(else_expression, visited)?,
                ),
            }),
            Expression::Sequence(expressions) => Some(Expression::Sequence(
                expressions
                    .iter()
                    .map(|expression| {
                        self.resolve_global_identifiers_in_expression(expression, visited)
                    })
                    .collect::<Option<Vec<_>>>()?,
            )),
            Expression::Member { object, property } => Some(Expression::Member {
                object: Box::new(self.resolve_global_identifiers_in_expression(object, visited)?),
                property: Box::new(
                    self.resolve_global_identifiers_in_expression(property, visited)?,
                ),
            }),
            Expression::Call { callee, arguments } => Some(Expression::Call {
                callee: Box::new(self.resolve_global_identifiers_in_expression(callee, visited)?),
                arguments: arguments
                    .iter()
                    .map(|argument| match argument {
                        CallArgument::Expression(expression) => Some(CallArgument::Expression(
                            self.resolve_global_identifiers_in_expression(expression, visited)?,
                        )),
                        CallArgument::Spread(expression) => Some(CallArgument::Spread(
                            self.resolve_global_identifiers_in_expression(expression, visited)?,
                        )),
                    })
                    .collect::<Option<Vec<_>>>()?,
            }),
            _ => Some(expression.clone()),
        }
    }

    pub(in crate::backend::direct_wasm) fn collect_string_concat_fragments(
        &self,
        expression: &Expression,
        fragments: &mut Vec<StringConcatFragment>,
    ) -> bool {
        if let Some(resolved) = self.resolve_bound_alias_expression(expression) {
            if !static_expression_matches(&resolved, expression) {
                if self
                    .resolve_single_char_code_expression(&resolved)
                    .is_some()
                {
                    fragments.push(StringConcatFragment::Dynamic(resolved));
                    return true;
                }
                return self.collect_string_concat_fragments(&resolved, fragments);
            }
        }

        if self
            .resolve_single_char_code_expression(expression)
            .is_some()
        {
            fragments.push(StringConcatFragment::Dynamic(expression.clone()));
            return true;
        }

        if let Expression::Binary {
            op: BinaryOp::Add,
            left,
            right,
        } = expression
        {
            return self.collect_string_concat_fragments(left, fragments)
                && self.collect_string_concat_fragments(right, fragments);
        }

        if let Some(text) = self.resolve_static_string_value(expression) {
            if let Some(StringConcatFragment::Static(existing)) = fragments.last_mut() {
                existing.push_str(&text);
            } else {
                fragments.push(StringConcatFragment::Static(text));
            }
            return true;
        }

        fragments.push(StringConcatFragment::Dynamic(expression.clone()));
        true
    }
}
