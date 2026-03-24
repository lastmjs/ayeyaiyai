use super::*;

impl DirectWasmCompiler {
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
}
