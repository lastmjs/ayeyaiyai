use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_array_slice_binding(
        &self,
        object: &Expression,
        arguments: &[CallArgument],
    ) -> Option<ArrayValueBinding> {
        let array_binding = self.resolve_array_binding_from_expression(object)?;
        let start = match arguments.first() {
            None => 0usize,
            Some(CallArgument::Expression(expression)) | Some(CallArgument::Spread(expression)) => {
                self.resolve_static_number_value(expression)?.max(0.0) as usize
            }
        };
        let end = match arguments.get(1) {
            None => array_binding.values.len(),
            Some(CallArgument::Expression(expression)) | Some(CallArgument::Spread(expression)) => {
                self.resolve_static_number_value(expression)?.max(0.0) as usize
            }
        };
        let start = start.min(array_binding.values.len());
        let end = end.min(array_binding.values.len()).max(start);
        Some(ArrayValueBinding {
            values: array_binding.values[start..end].to_vec(),
        })
    }

    pub(in crate::backend::direct_wasm) fn resolve_array_binding_from_expression(
        &self,
        expression: &Expression,
    ) -> Option<ArrayValueBinding> {
        if let Expression::Identifier(name) = expression {
            if let Some(binding) = self
                .local_typed_array_view_bindings
                .get(name)
                .and_then(|view| self.typed_array_view_static_values(view))
                .or_else(|| self.local_array_bindings.get(name).cloned())
                .or_else(|| {
                    let hidden_name = self.resolve_user_function_capture_hidden_name(name)?;
                    self.module.global_array_bindings.get(&hidden_name).cloned()
                })
                .or_else(|| self.module.global_array_bindings.get(name).cloned())
            {
                return Some(binding);
            }
        }

        if let Some(resolved) = self
            .resolve_bound_alias_expression(expression)
            .filter(|resolved| !static_expression_matches(resolved, expression))
        {
            return self.resolve_array_binding_from_expression(&resolved);
        }

        let binding = match expression {
            Expression::Assign { value, .. } | Expression::AssignSuperMember { value, .. } => {
                self.resolve_array_binding_from_expression(value)
            }
            Expression::Sequence(expressions) => expressions
                .last()
                .and_then(|expression| self.resolve_array_binding_from_expression(expression)),
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                let branch = if self.resolve_static_if_condition_value(condition)? {
                    then_expression.as_ref()
                } else {
                    else_expression.as_ref()
                };
                self.resolve_array_binding_from_expression(branch)
            }
            Expression::Identifier(name) => self
                .local_typed_array_view_bindings
                .get(name)
                .and_then(|view| self.typed_array_view_static_values(view))
                .or_else(|| self.local_array_bindings.get(name).cloned())
                .or_else(|| {
                    let hidden_name = self.resolve_user_function_capture_hidden_name(name)?;
                    self.module.global_array_bindings.get(&hidden_name).cloned()
                })
                .or_else(|| self.module.global_array_bindings.get(name).cloned()),
            Expression::EnumerateKeys(value) => self.resolve_enumerated_keys_binding(value),
            Expression::Member { object, property } => {
                let array_binding = self.resolve_array_binding_from_expression(object)?;
                let property = self
                    .resolve_property_key_expression(property)
                    .unwrap_or_else(|| self.materialize_static_expression(property));
                let index = argument_index_from_expression(&property)?;
                let value = array_binding.values.get(index as usize)?.clone()?;
                self.resolve_array_binding_from_expression(&value)
            }
            Expression::Call { callee, arguments } => {
                if let Some(binding) = self.resolve_builtin_array_call_binding(callee, arguments) {
                    return Some(binding);
                }
                if let Expression::Member { object, property } = callee.as_ref() {
                    if matches!(property.as_ref(), Expression::String(name) if name == "slice") {
                        return self.resolve_array_slice_binding(object, arguments);
                    }
                }
                let Expression::Identifier(name) = callee.as_ref() else {
                    return None;
                };
                let user_function = self.resolve_user_function_from_callee_name(name)?;
                let param_index = user_function.enumerated_keys_param_index?;
                let argument = match arguments.get(param_index) {
                    Some(CallArgument::Expression(expression))
                    | Some(CallArgument::Spread(expression)) => expression,
                    None => return Some(ArrayValueBinding { values: Vec::new() }),
                };
                self.resolve_enumerated_keys_binding(argument)
            }
            Expression::New { callee, arguments } => {
                if matches!(callee.as_ref(), Expression::Identifier(name) if name == "Array" && self.is_unshadowed_builtin_identifier(name))
                {
                    let expanded_arguments = self.expand_call_arguments(arguments);
                    if expanded_arguments.is_empty() {
                        return Some(ArrayValueBinding { values: Vec::new() });
                    }
                    if expanded_arguments.len() == 1
                        && let Some(length) =
                            self.resolve_static_number_value(&expanded_arguments[0])
                        && length.is_finite()
                        && length >= 0.0
                        && length.fract() == 0.0
                    {
                        return Some(ArrayValueBinding {
                            values: vec![None; length as usize],
                        });
                    }
                    return Some(ArrayValueBinding {
                        values: expanded_arguments.into_iter().map(Some).collect(),
                    });
                }
                let Expression::Identifier(name) = callee.as_ref() else {
                    return None;
                };
                let user_function = self.resolve_user_function_from_callee_name(name)?;
                let param_index = user_function.enumerated_keys_param_index?;
                let argument = match arguments.get(param_index) {
                    Some(CallArgument::Expression(expression))
                    | Some(CallArgument::Spread(expression)) => expression,
                    None => return Some(ArrayValueBinding { values: Vec::new() }),
                };
                self.resolve_enumerated_keys_binding(argument)
            }
            Expression::Array(elements) => {
                let mut values = Vec::new();
                for element in elements {
                    match element {
                        crate::ir::hir::ArrayElement::Expression(expression) => {
                            values.push(Some(self.materialize_static_expression(expression)));
                        }
                        crate::ir::hir::ArrayElement::Spread(expression) => {
                            if let Some(binding) =
                                self.resolve_array_binding_from_expression(expression)
                            {
                                values.extend(binding.values);
                            } else if let Some(binding) =
                                self.resolve_static_iterable_binding_from_expression(expression)
                            {
                                values.extend(binding.values);
                            } else {
                                values.push(Some(self.materialize_static_expression(expression)));
                            }
                        }
                    }
                }
                Some(ArrayValueBinding { values })
            }
            _ => None,
        };
        if binding.is_some() {
            return binding;
        }

        let materialized = self.materialize_static_expression(expression);
        if !static_expression_matches(&materialized, expression) {
            return self.resolve_array_binding_from_expression(&materialized);
        }
        None
    }

    pub(in crate::backend::direct_wasm) fn evaluate_simple_static_expression_with_bindings(
        &self,
        expression: &Expression,
        bindings: &HashMap<String, Expression>,
    ) -> Option<Expression> {
        match expression {
            Expression::Identifier(name) => Some(
                bindings
                    .get(name)
                    .cloned()
                    .unwrap_or_else(|| expression.clone()),
            ),
            Expression::Number(_)
            | Expression::BigInt(_)
            | Expression::String(_)
            | Expression::Bool(_)
            | Expression::Null
            | Expression::Undefined
            | Expression::This
            | Expression::NewTarget
            | Expression::Sent => Some(expression.clone()),
            Expression::Binary { op, left, right } => {
                let left = self.evaluate_simple_static_expression_with_bindings(left, bindings)?;
                let right =
                    self.evaluate_simple_static_expression_with_bindings(right, bindings)?;
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
                    BinaryOp::Equal => {
                        Some(Expression::Bool(static_expression_matches(&left, &right)))
                    }
                    BinaryOp::NotEqual => {
                        Some(Expression::Bool(!static_expression_matches(&left, &right)))
                    }
                    _ => None,
                }
            }
            Expression::Object(entries) => {
                let mut evaluated_entries = Vec::new();
                for entry in entries {
                    match entry {
                        ObjectEntry::Data { key, value } => {
                            evaluated_entries.push(ObjectEntry::Data {
                                key: self.evaluate_simple_static_expression_with_bindings(
                                    key, bindings,
                                )?,
                                value: self.evaluate_simple_static_expression_with_bindings(
                                    value, bindings,
                                )?,
                            })
                        }
                        _ => return None,
                    }
                }
                Some(Expression::Object(evaluated_entries))
            }
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn execute_simple_static_user_function_with_bindings(
        &self,
        function_name: &str,
        bindings: &HashMap<String, Expression>,
    ) -> Option<(Expression, HashMap<String, Expression>)> {
        let function = self.resolve_registered_function_declaration(function_name)?;
        let mut local_bindings = bindings.clone();
        for statement in &function.body {
            match statement {
                Statement::Var { name, value } | Statement::Let { name, value, .. } => {
                    let Some(value) = self
                        .evaluate_simple_static_expression_with_bindings(value, &local_bindings)
                    else {
                        return None;
                    };
                    local_bindings.insert(name.clone(), value);
                }
                Statement::Assign { name, value } => {
                    let Some(value) = self
                        .evaluate_simple_static_expression_with_bindings(value, &local_bindings)
                    else {
                        return None;
                    };
                    local_bindings.insert(name.clone(), value);
                }
                Statement::Return(value) => {
                    let Some(value) = self
                        .evaluate_simple_static_expression_with_bindings(value, &local_bindings)
                    else {
                        return None;
                    };
                    return Some((value, local_bindings));
                }
                Statement::Expression(expression) => {
                    if self
                        .evaluate_simple_static_expression_with_bindings(
                            expression,
                            &local_bindings,
                        )
                        .is_none()
                    {
                        return None;
                    }
                }
                Statement::Block { body } if body.is_empty() => {}
                _ => return None,
            }
        }
        None
    }

    pub(in crate::backend::direct_wasm) fn analyze_simple_generator_statements(
        &self,
        statements: &[Statement],
        steps: &mut Vec<SimpleGeneratorStep>,
        effects: &mut Vec<Statement>,
    ) -> Option<()> {
        for statement in statements {
            match statement {
                Statement::Yield { value } => {
                    steps.push(SimpleGeneratorStep {
                        effects: std::mem::take(effects),
                        outcome: SimpleGeneratorStepOutcome::Yield(value.clone()),
                    });
                }
                Statement::Throw(value) => {
                    steps.push(SimpleGeneratorStep {
                        effects: std::mem::take(effects),
                        outcome: SimpleGeneratorStepOutcome::Throw(value.clone()),
                    });
                    return Some(());
                }
                Statement::Block { body } => {
                    self.analyze_simple_generator_statements(body, steps, effects)?;
                }
                Statement::Var { .. } | Statement::Let { .. } => {
                    effects.push(statement.clone());
                }
                Statement::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    let materialized_condition = self.materialize_static_expression(condition);
                    let branch =
                        if self.resolve_static_if_condition_value(&materialized_condition)? {
                            then_branch
                        } else {
                            else_branch
                        };
                    self.analyze_simple_generator_statements(branch, steps, effects)?;
                }
                Statement::Assign { .. }
                | Statement::AssignMember { .. }
                | Statement::Expression(_)
                | Statement::Print { .. } => effects.push(statement.clone()),
                _ => return None,
            }
        }

        Some(())
    }

    pub(in crate::backend::direct_wasm) fn simple_generator_call_arguments(
        &self,
        call_argument_values: &[Expression],
    ) -> Vec<CallArgument> {
        call_argument_values
            .iter()
            .cloned()
            .map(CallArgument::Expression)
            .collect()
    }

    pub(in crate::backend::direct_wasm) fn simple_generator_arguments_binding_expression(
        &self,
        arguments_values: &[Expression],
    ) -> Expression {
        Expression::Array(
            arguments_values
                .iter()
                .cloned()
                .map(crate::ir::hir::ArrayElement::Expression)
                .collect(),
        )
    }

    pub(in crate::backend::direct_wasm) fn simple_generator_arguments_are_shadowed(
        &self,
        user_function: &UserFunction,
    ) -> bool {
        user_function.body_declares_arguments_binding
            || user_function
                .params
                .iter()
                .any(|param| param == "arguments")
    }

    pub(in crate::backend::direct_wasm) fn update_simple_generator_call_frame_state(
        &self,
        original_statement: &Statement,
        transformed_statement: &Statement,
        user_function: &UserFunction,
        mapped_arguments: bool,
        call_argument_values: &mut Vec<Expression>,
        arguments_values: &mut Vec<Expression>,
    ) {
        if let Statement::Assign { name, value } = transformed_statement
            && let Some(index) = user_function.params.iter().position(|param| param == name)
        {
            if index >= call_argument_values.len() {
                call_argument_values.resize(index + 1, Expression::Undefined);
            }
            call_argument_values[index] = value.clone();
            if mapped_arguments && index < arguments_values.len() {
                arguments_values[index] = value.clone();
            }
            return;
        }

        let Statement::AssignMember {
            object: original_object,
            ..
        } = original_statement
        else {
            return;
        };
        if self.simple_generator_arguments_are_shadowed(user_function)
            || !matches!(original_object, Expression::Identifier(name) if name == "arguments")
        {
            return;
        }
        let Statement::AssignMember {
            property, value, ..
        } = transformed_statement
        else {
            return;
        };
        let Some(index) = argument_index_from_expression(property).map(|index| index as usize)
        else {
            return;
        };
        if index >= arguments_values.len() {
            arguments_values.resize(index + 1, Expression::Undefined);
        }
        arguments_values[index] = value.clone();
        if mapped_arguments
            && index < user_function.params.len()
            && index < call_argument_values.len()
        {
            call_argument_values[index] = value.clone();
        }
    }
}
