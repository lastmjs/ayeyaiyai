use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn execute_bound_snapshot_statements(
        &self,
        statements: &[Statement],
        bindings: &mut HashMap<String, Expression>,
        current_function_name: Option<&str>,
    ) -> Option<BoundSnapshotControlFlow> {
        for statement in statements {
            match statement {
                Statement::Block { body } => {
                    if let Some(result) = self.execute_bound_snapshot_statements(
                        body,
                        bindings,
                        current_function_name,
                    ) && !matches!(result, BoundSnapshotControlFlow::None)
                    {
                        return Some(result);
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
                    if let Some(result) = self.execute_bound_snapshot_statements(
                        branch,
                        bindings,
                        current_function_name,
                    ) && !matches!(result, BoundSnapshotControlFlow::None)
                    {
                        return Some(result);
                    }
                }
                Statement::Return(value) => {
                    return Some(BoundSnapshotControlFlow::Return(
                        self.evaluate_bound_snapshot_expression(
                            value,
                            bindings,
                            current_function_name,
                        )?,
                    ));
                }
                Statement::Throw(value) => {
                    let throw_value = if let Expression::Identifier(name) = value {
                        Expression::Identifier(
                            self.resolve_bound_snapshot_binding_name(name, bindings)
                                .to_string(),
                        )
                    } else {
                        self.evaluate_bound_snapshot_expression(
                            value,
                            bindings,
                            current_function_name,
                        )?
                    };
                    return Some(BoundSnapshotControlFlow::Throw(throw_value));
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
        Some(BoundSnapshotControlFlow::None)
    }

    pub(in crate::backend::direct_wasm) fn apply_bound_snapshot_member_assignment(
        &self,
        object: &Expression,
        property: &Expression,
        value: &Expression,
        bindings: &mut HashMap<String, Expression>,
        current_function_name: Option<&str>,
    ) -> Option<Expression> {
        let binding_names = match object {
            Expression::Identifier(object_name) => vec![
                self.resolve_bound_snapshot_binding_name(object_name, bindings)
                    .to_string(),
            ],
            Expression::This => {
                let this_binding = bindings.get("this").cloned()?;
                match this_binding {
                    Expression::Identifier(object_name) => vec![
                        self.resolve_bound_snapshot_binding_name(&object_name, bindings)
                            .to_string(),
                    ],
                    _ => vec!["this".to_string()],
                }
            }
            _ => return None,
        };
        let property =
            self.evaluate_bound_snapshot_expression(property, bindings, current_function_name)?;
        let value =
            self.evaluate_bound_snapshot_expression(value, bindings, current_function_name)?;
        let current_object = binding_names
            .iter()
            .find_map(|object_name| bindings.get(object_name).cloned())
            .unwrap_or_else(|| {
                self.evaluate_bound_snapshot_expression(object, bindings, current_function_name)
                    .unwrap_or(Expression::Undefined)
            });
        let mut object_binding = self.resolve_object_binding_from_expression(&current_object)?;
        object_binding_set_property(&mut object_binding, property, value.clone());
        let updated_object = object_binding_to_expression(&object_binding);
        for object_name in binding_names {
            bindings.insert(object_name, updated_object.clone());
        }
        Some(value)
    }

    pub(in crate::backend::direct_wasm) fn bound_snapshot_array_expression(
        &self,
        expression: &Expression,
        bindings: &HashMap<String, Expression>,
    ) -> Option<Vec<ArrayElement>> {
        match expression {
            Expression::Array(elements) => Some(elements.clone()),
            Expression::Identifier(name) => {
                let resolved_name = self.resolve_bound_snapshot_binding_name(name, bindings);
                if let Some(Expression::Array(elements)) = bindings.get(resolved_name) {
                    return Some(elements.clone());
                }
                let array_binding = self.resolve_array_binding_from_expression(
                    &Expression::Identifier(resolved_name.to_string()),
                )?;
                Some(
                    array_binding
                        .values
                        .into_iter()
                        .map(|value| {
                            ArrayElement::Expression(value.unwrap_or(Expression::Undefined))
                        })
                        .collect(),
                )
            }
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn apply_bound_snapshot_array_push(
        &self,
        object: &Expression,
        arguments: &[CallArgument],
        bindings: &mut HashMap<String, Expression>,
        current_function_name: Option<&str>,
    ) -> Option<Expression> {
        let resolved_object_name = match object {
            Expression::Identifier(name) => Some(
                self.resolve_bound_snapshot_binding_name(name, bindings)
                    .to_string(),
            ),
            _ => None,
        };
        let object_value =
            self.evaluate_bound_snapshot_expression(object, bindings, current_function_name)?;
        let mut elements = self.bound_snapshot_array_expression(&object_value, bindings)?;
        for argument in arguments {
            match argument {
                CallArgument::Expression(expression) => {
                    let value = self.evaluate_bound_snapshot_expression(
                        expression,
                        bindings,
                        current_function_name,
                    )?;
                    elements.push(ArrayElement::Expression(value));
                }
                CallArgument::Spread(expression) => {
                    let value = self.evaluate_bound_snapshot_expression(
                        expression,
                        bindings,
                        current_function_name,
                    )?;
                    let spread_elements = self.bound_snapshot_array_expression(&value, bindings)?;
                    for element in spread_elements {
                        let ArrayElement::Expression(value) = element else {
                            return None;
                        };
                        elements.push(ArrayElement::Expression(value));
                    }
                }
            }
        }
        if let Some(resolved_object_name) = resolved_object_name {
            bindings.insert(resolved_object_name, Expression::Array(elements.clone()));
        }
        Some(Expression::Number(elements.len() as f64))
    }
}
