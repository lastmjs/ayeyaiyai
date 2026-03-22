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

    pub(in crate::backend::direct_wasm) fn analyze_simple_generator_function(
        &self,
        function_name: &str,
    ) -> Option<(Vec<SimpleGeneratorStep>, Vec<Statement>)> {
        let function = self.resolve_registered_function_declaration(function_name)?;
        if !matches!(function.kind, FunctionKind::Generator)
            || !function.params.is_empty()
            || function.body.is_empty()
        {
            return None;
        }

        let mut steps = Vec::new();
        let mut effects = Vec::new();
        self.analyze_simple_generator_statements(&function.body, &mut steps, &mut effects)?;

        Some((steps, effects))
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

    pub(in crate::backend::direct_wasm) fn substitute_simple_generator_statement_call_frame_bindings(
        &self,
        statement: &Statement,
        user_function: &UserFunction,
        this_binding: &Expression,
        arguments_binding: &Expression,
    ) -> Option<Statement> {
        Some(match statement {
            Statement::Block { body } => Statement::Block {
                body: body
                    .iter()
                    .map(|statement| {
                        self.substitute_simple_generator_statement_call_frame_bindings(
                            statement,
                            user_function,
                            this_binding,
                            arguments_binding,
                        )
                    })
                    .collect::<Option<Vec<_>>>()?,
            },
            Statement::Assign { name, value } => Statement::Assign {
                name: name.clone(),
                value: self.substitute_call_frame_special_bindings(
                    value,
                    user_function,
                    this_binding,
                    arguments_binding,
                ),
            },
            Statement::AssignMember {
                object,
                property,
                value,
            } => Statement::AssignMember {
                object: self.substitute_call_frame_special_bindings(
                    object,
                    user_function,
                    this_binding,
                    arguments_binding,
                ),
                property: self.substitute_call_frame_special_bindings(
                    property,
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
            },
            Statement::Print { values } => Statement::Print {
                values: values
                    .iter()
                    .map(|value| {
                        self.substitute_call_frame_special_bindings(
                            value,
                            user_function,
                            this_binding,
                            arguments_binding,
                        )
                    })
                    .collect(),
            },
            Statement::Expression(expression) => {
                Statement::Expression(self.substitute_call_frame_special_bindings(
                    expression,
                    user_function,
                    this_binding,
                    arguments_binding,
                ))
            }
            Statement::Throw(value) => {
                Statement::Throw(self.substitute_call_frame_special_bindings(
                    value,
                    user_function,
                    this_binding,
                    arguments_binding,
                ))
            }
            Statement::Yield { value } => Statement::Yield {
                value: self.substitute_call_frame_special_bindings(
                    value,
                    user_function,
                    this_binding,
                    arguments_binding,
                ),
            },
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => Statement::If {
                condition: self.substitute_call_frame_special_bindings(
                    condition,
                    user_function,
                    this_binding,
                    arguments_binding,
                ),
                then_branch: then_branch
                    .iter()
                    .map(|statement| {
                        self.substitute_simple_generator_statement_call_frame_bindings(
                            statement,
                            user_function,
                            this_binding,
                            arguments_binding,
                        )
                    })
                    .collect::<Option<Vec<_>>>()?,
                else_branch: else_branch
                    .iter()
                    .map(|statement| {
                        self.substitute_simple_generator_statement_call_frame_bindings(
                            statement,
                            user_function,
                            this_binding,
                            arguments_binding,
                        )
                    })
                    .collect::<Option<Vec<_>>>()?,
            },
            _ => return None,
        })
    }

    pub(in crate::backend::direct_wasm) fn resolve_array_prototype_simple_generator_source(
        &self,
        expression: &Expression,
    ) -> Option<(Vec<SimpleGeneratorStep>, Vec<Statement>)> {
        let materialized = self.materialize_static_expression(expression);
        if !static_expression_matches(&materialized, expression) {
            return self.resolve_array_prototype_simple_generator_source(&materialized);
        }
        self.resolve_array_binding_from_expression(expression)?;

        let iterator_property = self.materialize_static_expression(&symbol_iterator_expression());
        let array_prototype = Expression::Member {
            object: Box::new(Expression::Identifier("Array".to_string())),
            property: Box::new(Expression::String("prototype".to_string())),
        };
        let LocalFunctionBinding::User(function_name) =
            self.resolve_member_function_binding(&array_prototype, &iterator_property)?
        else {
            return None;
        };
        let user_function = self.module.user_function_map.get(&function_name)?;
        if !user_function.is_generator()
            || !user_function.params.is_empty()
            || user_function.has_parameter_defaults()
            || !user_function.extra_argument_indices.is_empty()
        {
            return None;
        }

        let function = self.resolve_registered_function_declaration(&function_name)?;
        let arguments_binding = Expression::Identifier("arguments".to_string());
        let analysis_this_binding = if self
            .runtime_array_length_local_for_expression(expression)
            .is_some()
        {
            let array_binding = self.resolve_array_binding_from_expression(expression)?;
            Expression::Array(
                array_binding
                    .values
                    .into_iter()
                    .map(|value| {
                        crate::ir::hir::ArrayElement::Expression(
                            value.unwrap_or(Expression::Undefined),
                        )
                    })
                    .collect(),
            )
        } else {
            expression.clone()
        };
        let substituted_body = function
            .body
            .iter()
            .map(|statement| {
                self.substitute_simple_generator_statement_call_frame_bindings(
                    statement,
                    user_function,
                    &analysis_this_binding,
                    &arguments_binding,
                )
            })
            .collect::<Option<Vec<_>>>()?;
        let mut steps = Vec::new();
        let mut effects = Vec::new();
        self.analyze_simple_generator_statements(&substituted_body, &mut steps, &mut effects)?;
        Some((steps, effects))
    }

    pub(in crate::backend::direct_wasm) fn resolve_simple_generator_source(
        &self,
        expression: &Expression,
    ) -> Option<(Vec<SimpleGeneratorStep>, Vec<Statement>)> {
        let materialized = self.materialize_static_expression(expression);
        if !static_expression_matches(&materialized, expression) {
            return self.resolve_simple_generator_source(&materialized);
        }

        let Expression::Call { callee, arguments } = expression else {
            return None;
        };
        if !arguments.is_empty() {
            return None;
        }

        let LocalFunctionBinding::User(function_name) =
            self.resolve_function_binding_from_expression(callee)?
        else {
            return None;
        };
        let user_function = self.module.user_function_map.get(&function_name)?;
        if !user_function.is_generator()
            || !user_function.params.is_empty()
            || user_function.has_parameter_defaults()
            || !user_function.extra_argument_indices.is_empty()
        {
            return None;
        }

        self.analyze_simple_generator_function(&function_name)
    }

    pub(in crate::backend::direct_wasm) fn analyze_effectful_iterator_source_call(
        &self,
        expression: &Expression,
    ) -> Option<(String, Expression, Vec<Statement>)> {
        let materialized = self.materialize_static_expression(expression);
        if !static_expression_matches(&materialized, expression) {
            return self.analyze_effectful_iterator_source_call(&materialized);
        }

        let Expression::Call { callee, arguments } = expression else {
            return None;
        };
        let LocalFunctionBinding::User(function_name) =
            self.resolve_function_binding_from_expression(callee)?
        else {
            return None;
        };
        let user_function = self.module.user_function_map.get(&function_name)?;
        if user_function.is_async()
            || user_function.is_generator()
            || user_function.has_parameter_defaults()
            || !user_function.extra_argument_indices.is_empty()
        {
            return None;
        }

        let function = self.resolve_registered_function_declaration(&function_name)?;
        let (terminal_statement, effect_statements) = function.body.split_last()?;
        let mut substituted_effects = Vec::new();
        for statement in effect_statements {
            match statement {
                Statement::Assign { name, value } => {
                    let substituted = self.substitute_user_function_argument_bindings(
                        value,
                        user_function,
                        arguments,
                    );
                    if expression_mentions_call_frame_state(&substituted) {
                        return None;
                    }
                    substituted_effects.push(Statement::Assign {
                        name: name.clone(),
                        value: substituted,
                    });
                }
                Statement::Expression(Expression::Update { name, op, prefix }) => {
                    substituted_effects.push(Statement::Expression(Expression::Update {
                        name: name.clone(),
                        op: *op,
                        prefix: *prefix,
                    }));
                }
                Statement::Expression(effect_expression) => {
                    let substituted = self.substitute_user_function_argument_bindings(
                        effect_expression,
                        user_function,
                        arguments,
                    );
                    if expression_mentions_call_frame_state(&substituted) {
                        return None;
                    }
                    substituted_effects.push(Statement::Expression(substituted));
                }
                Statement::Block { body } if body.is_empty() => {}
                _ => return None,
            }
        }

        let Statement::Return(return_value) = terminal_statement else {
            return None;
        };
        let returned_expression =
            self.substitute_user_function_argument_bindings(return_value, user_function, arguments);
        if expression_mentions_call_frame_state(&returned_expression)
            || static_expression_matches(&returned_expression, expression)
        {
            return None;
        }

        Some((function_name, returned_expression, substituted_effects))
    }

    pub(in crate::backend::direct_wasm) fn resolve_static_iterable_binding_from_expression(
        &self,
        expression: &Expression,
    ) -> Option<ArrayValueBinding> {
        let object_binding = self.resolve_object_binding_from_expression(expression)?;
        let symbol_iterator = self.materialize_static_expression(&Expression::Member {
            object: Box::new(Expression::Identifier("Symbol".to_string())),
            property: Box::new(Expression::String("iterator".to_string())),
        });
        let iterator_method =
            object_binding_lookup_value(&object_binding, &symbol_iterator)?.clone();
        let LocalFunctionBinding::User(iterator_function_name) =
            self.resolve_function_binding_from_expression(&iterator_method)?
        else {
            return None;
        };
        let (iterator_result, iterator_bindings) = self
            .execute_simple_static_user_function_with_bindings(
                &iterator_function_name,
                &HashMap::new(),
            )?;
        let iterator_result_binding =
            self.resolve_object_binding_from_expression(&iterator_result)?;
        let next_value = object_binding_lookup_value(
            &iterator_result_binding,
            &Expression::String("next".to_string()),
        )?
        .clone();
        let LocalFunctionBinding::User(next_function_name) =
            self.resolve_function_binding_from_expression(&next_value)?
        else {
            return None;
        };

        let mut step_bindings = iterator_bindings;
        let mut values = Vec::new();
        for _ in 0..256 {
            let (step_result, updated_bindings) = self
                .execute_simple_static_user_function_with_bindings(
                    &next_function_name,
                    &step_bindings,
                )?;
            step_bindings = updated_bindings;
            let step_object_binding = self.resolve_object_binding_from_expression(&step_result)?;
            let done = object_binding_lookup_value(
                &step_object_binding,
                &Expression::String("done".to_string()),
            )
            .cloned()
            .unwrap_or(Expression::Bool(false));
            let value = object_binding_lookup_value(
                &step_object_binding,
                &Expression::String("value".to_string()),
            )
            .cloned()
            .unwrap_or(Expression::Undefined);
            match done {
                Expression::Bool(true) => return Some(ArrayValueBinding { values }),
                Expression::Bool(false) => values.push(Some(value)),
                _ => return None,
            }
        }

        None
    }

    pub(in crate::backend::direct_wasm) fn ensure_runtime_array_length_local(
        &mut self,
        name: &str,
    ) -> u32 {
        if let Some(local) = self.runtime_array_length_locals.get(name).copied() {
            return local;
        }
        let local = self.allocate_temp_local();
        self.runtime_array_length_locals
            .insert(name.to_string(), local);
        local
    }

    pub(in crate::backend::direct_wasm) fn resolve_runtime_array_binding_name(
        &self,
        name: &str,
    ) -> Option<String> {
        if self.local_array_bindings.contains_key(name)
            || self.runtime_array_length_locals.contains_key(name)
            || self.runtime_array_slots.contains_key(name)
        {
            return Some(name.to_string());
        }
        let (resolved_name, _) = self.resolve_current_local_binding(name)?;
        if self.local_array_bindings.contains_key(&resolved_name)
            || self
                .runtime_array_length_locals
                .contains_key(&resolved_name)
            || self.runtime_array_slots.contains_key(&resolved_name)
        {
            return Some(resolved_name);
        }
        None
    }

    pub(in crate::backend::direct_wasm) fn resolve_local_array_iterator_binding_name(
        &self,
        name: &str,
    ) -> Option<String> {
        if self.local_array_iterator_bindings.contains_key(name) {
            return Some(name.to_string());
        }
        let (resolved_name, _) = self.resolve_current_local_binding(name)?;
        self.local_array_iterator_bindings
            .contains_key(&resolved_name)
            .then_some(resolved_name)
    }

    pub(in crate::backend::direct_wasm) fn runtime_array_length_local_for_expression(
        &self,
        expression: &Expression,
    ) -> Option<u32> {
        let Expression::Identifier(name) = expression else {
            return None;
        };
        let binding_name = self
            .resolve_runtime_array_binding_name(name)
            .unwrap_or_else(|| name.clone());
        self.runtime_array_length_locals.get(&binding_name).copied()
    }

    pub(in crate::backend::direct_wasm) fn ensure_runtime_array_slots_for_binding(
        &mut self,
        name: &str,
        binding: &ArrayValueBinding,
    ) {
        for index in 0..TRACKED_ARRAY_SLOT_LIMIT {
            let slot = if let Some(slot) = self.runtime_array_slot(name, index) {
                slot
            } else {
                let slot = RuntimeArraySlot {
                    value_local: self.allocate_temp_local(),
                    present_local: self.allocate_temp_local(),
                };
                self.runtime_array_slots
                    .entry(name.to_string())
                    .or_default()
                    .insert(index, slot.clone());
                slot
            };
            match binding.values.get(index as usize).cloned().flatten() {
                Some(value) => {
                    self.emit_numeric_expression(&value)
                        .expect("runtime array slot initialization is supported");
                    self.push_local_set(slot.value_local);
                    self.push_i32_const(1);
                    self.push_local_set(slot.present_local);
                }
                None => {
                    self.push_i32_const(JS_UNDEFINED_TAG);
                    self.push_local_set(slot.value_local);
                    self.push_i32_const(0);
                    self.push_local_set(slot.present_local);
                }
            }
        }
    }

    pub(in crate::backend::direct_wasm) fn runtime_array_slot(
        &self,
        name: &str,
        index: u32,
    ) -> Option<RuntimeArraySlot> {
        self.runtime_array_slots
            .get(name)
            .and_then(|slots| slots.get(&index))
            .cloned()
    }

    pub(in crate::backend::direct_wasm) fn emit_runtime_array_slot_read(
        &mut self,
        name: &str,
        index: u32,
    ) -> DirectResult<bool> {
        let binding_name = self
            .resolve_runtime_array_binding_name(name)
            .unwrap_or_else(|| name.to_string());
        let Some(slot) = self.runtime_array_slot(&binding_name, index) else {
            return Ok(false);
        };
        self.push_local_get(slot.present_local);
        self.instructions.push(0x04);
        self.instructions.push(I32_TYPE);
        self.push_control_frame();
        self.push_local_get(slot.value_local);
        self.instructions.push(0x05);
        self.push_i32_const(JS_UNDEFINED_TAG);
        self.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_runtime_array_slot_write_from_local(
        &mut self,
        name: &str,
        index: u32,
        value_local: u32,
    ) -> DirectResult<bool> {
        let binding_name = self
            .resolve_runtime_array_binding_name(name)
            .unwrap_or_else(|| name.to_string());
        let Some(slot) = self.runtime_array_slot(&binding_name, index) else {
            return Ok(false);
        };
        self.push_local_get(value_local);
        self.push_local_set(slot.value_local);
        self.push_i32_const(1);
        self.push_local_set(slot.present_local);
        if let Some(length_local) = self.runtime_array_length_locals.get(&binding_name).copied() {
            let next_length = index as i32 + 1;
            self.push_local_get(length_local);
            self.push_i32_const(next_length);
            self.push_binary_op(BinaryOp::LessThan)?;
            self.instructions.push(0x04);
            self.instructions.push(EMPTY_BLOCK_TYPE);
            self.push_control_frame();
            self.push_i32_const(next_length);
            self.push_local_set(length_local);
            self.instructions.push(0x0b);
            self.pop_control_frame();
        }
        self.push_local_get(value_local);
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn clear_runtime_array_slot(
        &mut self,
        name: &str,
        index: u32,
    ) -> bool {
        let binding_name = self
            .resolve_runtime_array_binding_name(name)
            .unwrap_or_else(|| name.to_string());
        let Some(slot) = self.runtime_array_slot(&binding_name, index) else {
            return false;
        };
        self.push_i32_const(JS_UNDEFINED_TAG);
        self.push_local_set(slot.value_local);
        self.push_i32_const(0);
        self.push_local_set(slot.present_local);
        true
    }

    pub(in crate::backend::direct_wasm) fn emit_dynamic_runtime_array_slot_write(
        &mut self,
        name: &str,
        property: &Expression,
        value: &Expression,
    ) -> DirectResult<bool> {
        let binding_name = self
            .resolve_runtime_array_binding_name(name)
            .unwrap_or_else(|| name.to_string());
        let Some(indices) = self
            .runtime_array_slots
            .get(&binding_name)
            .map(|slots| slots.keys().copied().collect::<Vec<_>>())
        else {
            return Ok(false);
        };

        let property_local = self.allocate_temp_local();
        self.emit_numeric_expression(property)?;
        self.push_local_set(property_local);

        let value_local = self.allocate_temp_local();
        self.emit_numeric_expression(value)?;
        self.push_local_set(value_local);

        let mut sorted_indices = indices;
        sorted_indices.sort_unstable();
        let mut open_frames = 0;
        for index in sorted_indices {
            self.push_local_get(property_local);
            self.push_i32_const(index as i32);
            self.push_binary_op(BinaryOp::Equal)?;
            self.instructions.push(0x04);
            self.instructions.push(I32_TYPE);
            self.push_control_frame();
            open_frames += 1;
            self.update_tracked_array_specialized_function_value(&binding_name, index, value)?;
            if !self.emit_runtime_array_slot_write_from_local(&binding_name, index, value_local)? {
                self.push_local_get(value_local);
            }
            self.instructions.push(0x05);
        }

        self.push_local_get(value_local);
        for _ in 0..open_frames {
            self.instructions.push(0x0b);
            self.pop_control_frame();
        }
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_runtime_array_push_from_local(
        &mut self,
        name: &str,
        value_local: u32,
        value_expression: &Expression,
    ) -> DirectResult<bool> {
        let binding_name = self
            .resolve_runtime_array_binding_name(name)
            .unwrap_or_else(|| name.to_string());
        let Some(length_local) = self.runtime_array_length_locals.get(&binding_name).copied()
        else {
            return Ok(false);
        };
        if binding_name.starts_with("__ayy_array_rest_")
            && let Expression::Member { object, property } = value_expression
            && matches!(property.as_ref(), Expression::String(property_name) if property_name == "value")
            && let Some(IteratorStepBinding::Runtime { done_local, .. }) =
                self.resolve_iterator_step_binding_from_expression(object)
        {
            self.push_local_get(done_local);
            self.instructions.push(0x04);
            self.instructions.push(I32_TYPE);
            self.push_control_frame();
            self.push_local_get(length_local);
            self.instructions.push(0x05);
            self.emit_runtime_array_push_with_length_local(
                &binding_name,
                length_local,
                value_local,
                value_expression,
            )?;
            self.instructions.push(0x0b);
            self.pop_control_frame();
            return Ok(true);
        }
        self.emit_runtime_array_push_with_length_local(
            &binding_name,
            length_local,
            value_local,
            value_expression,
        )
    }

    pub(in crate::backend::direct_wasm) fn emit_runtime_array_push_with_length_local(
        &mut self,
        name: &str,
        length_local: u32,
        value_local: u32,
        value_expression: &Expression,
    ) -> DirectResult<bool> {
        let Some(indices) = self
            .runtime_array_slots
            .get(name)
            .map(|slots| slots.keys().copied().collect::<Vec<_>>())
        else {
            self.push_local_get(length_local);
            self.push_i32_const(1);
            self.push_binary_op(BinaryOp::Add)?;
            self.push_local_tee(length_local);
            return Ok(true);
        };

        let mut sorted_indices = indices;
        sorted_indices.sort_unstable();
        let mut open_frames = 0;
        let matched_local = self.allocate_temp_local();
        self.push_i32_const(0);
        self.push_local_set(matched_local);
        for index in sorted_indices {
            self.push_local_get(length_local);
            self.push_i32_const(index as i32);
            self.push_binary_op(BinaryOp::Equal)?;
            self.instructions.push(0x04);
            self.instructions.push(EMPTY_BLOCK_TYPE);
            self.push_control_frame();
            open_frames += 1;
            self.update_tracked_array_specialized_function_value(name, index, value_expression)?;
            if self.emit_runtime_array_slot_write_from_local(name, index, value_local)? {
                self.instructions.push(0x1a);
            }
            self.push_i32_const(1);
            self.push_local_set(matched_local);
            self.instructions.push(0x05);
        }
        self.push_local_get(matched_local);
        self.instructions.push(0x04);
        self.instructions.push(EMPTY_BLOCK_TYPE);
        self.push_control_frame();
        self.instructions.push(0x05);
        self.push_local_get(length_local);
        self.push_i32_const(1);
        self.push_binary_op(BinaryOp::Add)?;
        self.push_local_set(length_local);
        self.instructions.push(0x0b);
        self.pop_control_frame();
        for _ in 0..open_frames {
            self.instructions.push(0x0b);
            self.pop_control_frame();
        }
        self.push_local_get(length_local);
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn tracked_direct_arguments_prefix_len(&self) -> u32 {
        let mut indices = self.arguments_slots.keys().copied().collect::<Vec<_>>();
        indices.sort_unstable();
        let mut next_index = 0;
        for index in indices {
            if index != next_index {
                break;
            }
            next_index += 1;
        }
        next_index
    }

    pub(in crate::backend::direct_wasm) fn resolve_iterator_source_kind(
        &self,
        expression: &Expression,
    ) -> Option<IteratorSourceKind> {
        let materialized = self.materialize_static_expression(expression);
        if !static_expression_matches(&materialized, expression) {
            return self.resolve_iterator_source_kind(&materialized);
        }
        if self.is_direct_arguments_object(expression) {
            return Some(IteratorSourceKind::DirectArguments {
                tracked_prefix_len: self.tracked_direct_arguments_prefix_len(),
            });
        }
        if let Expression::Identifier(name) = expression {
            if self.local_typed_array_view_bindings.contains_key(name) {
                return Some(IteratorSourceKind::TypedArrayView { name: name.clone() });
            }
        }
        if let Expression::Call { callee, arguments } = expression {
            if arguments.is_empty() {
                if let Expression::Member { object, property } = callee.as_ref() {
                    if matches!(property.as_ref(), Expression::String(name) if name == "keys") {
                        let array_binding = self.resolve_array_binding_from_expression(object)?;
                        return Some(IteratorSourceKind::StaticArray {
                            values: array_binding.values,
                            keys_only: true,
                            length_local: self.runtime_array_length_local_for_expression(object),
                            runtime_name: match object.as_ref() {
                                Expression::Identifier(name) => Some(name.clone()),
                                _ => None,
                            },
                        });
                    }
                }
            }
        }
        if let Some((steps, completion_effects)) =
            self.resolve_array_prototype_simple_generator_source(expression)
        {
            return Some(IteratorSourceKind::SimpleGenerator {
                steps,
                completion_effects,
            });
        }
        if let Some(array_binding) = self.resolve_array_binding_from_expression(expression) {
            return Some(IteratorSourceKind::StaticArray {
                values: array_binding.values,
                keys_only: false,
                length_local: self.runtime_array_length_local_for_expression(expression),
                runtime_name: match expression {
                    Expression::Identifier(name) => Some(name.clone()),
                    _ => None,
                },
            });
        }
        if let Some((steps, completion_effects)) = self.resolve_simple_generator_source(expression)
        {
            return Some(IteratorSourceKind::SimpleGenerator {
                steps,
                completion_effects,
            });
        }
        if let Some((_, returned_expression, _)) =
            self.analyze_effectful_iterator_source_call(expression)
        {
            return self.resolve_iterator_source_kind(&returned_expression);
        }
        let binding = self.resolve_static_iterable_binding_from_expression(expression)?;
        Some(IteratorSourceKind::StaticArray {
            values: binding.values,
            keys_only: false,
            length_local: None,
            runtime_name: None,
        })
    }

    pub(in crate::backend::direct_wasm) fn update_local_array_iterator_binding(
        &mut self,
        name: &str,
        value: &Expression,
    ) {
        let Expression::GetIterator(iterated) = value else {
            self.local_array_iterator_bindings.remove(name);
            return;
        };
        let Some(source) = self.resolve_iterator_source_kind(iterated) else {
            self.local_array_iterator_bindings.remove(name);
            return;
        };
        let index_local = self
            .resolve_local_array_iterator_binding_name(name)
            .and_then(|binding_name| self.local_array_iterator_bindings.get(&binding_name))
            .map(|binding| binding.index_local)
            .unwrap_or_else(|| self.allocate_temp_local());
        let static_index = match &source {
            IteratorSourceKind::StaticArray { length_local, .. }
                if length_local.is_none() || name.starts_with("__ayy_array_iter_") =>
            {
                Some(0)
            }
            _ => None,
        };
        self.local_array_iterator_bindings.insert(
            name.to_string(),
            ArrayIteratorBinding {
                source,
                index_local,
                static_index,
            },
        );
        self.push_i32_const(0);
        self.push_local_set(index_local);
        self.local_kinds
            .insert(name.to_string(), StaticValueKind::Object);
    }

    pub(in crate::backend::direct_wasm) fn update_local_iterator_step_binding(
        &mut self,
        name: &str,
        value: &Expression,
    ) {
        let Expression::Call { callee, arguments } = value else {
            self.local_iterator_step_bindings.remove(name);
            return;
        };
        if !arguments.is_empty() {
            self.local_iterator_step_bindings.remove(name);
            return;
        }
        let Expression::Member { object, property } = callee.as_ref() else {
            self.local_iterator_step_bindings.remove(name);
            return;
        };
        if !matches!(property.as_ref(), Expression::String(property_name) if property_name == "next")
        {
            self.local_iterator_step_bindings.remove(name);
            return;
        }
        let Expression::Identifier(iterator_name) = object.as_ref() else {
            self.local_iterator_step_bindings.remove(name);
            return;
        };
        let iterator_binding_name = self
            .resolve_local_array_iterator_binding_name(iterator_name)
            .unwrap_or_else(|| iterator_name.clone());
        let Some(mut iterator_binding) = self
            .local_array_iterator_bindings
            .get(&iterator_binding_name)
            .cloned()
        else {
            self.local_iterator_step_bindings.remove(name);
            return;
        };
        let (done_local, value_local) = match self.local_iterator_step_bindings.get(name) {
            Some(IteratorStepBinding::Runtime {
                done_local,
                value_local,
                ..
            }) => (*done_local, *value_local),
            _ => (self.allocate_temp_local(), self.allocate_temp_local()),
        };
        let function_binding = match &iterator_binding.source {
            IteratorSourceKind::StaticArray {
                values, keys_only, ..
            } if !keys_only => {
                let bindings = values
                    .iter()
                    .flatten()
                    .map(|value| self.resolve_function_binding_from_expression(value))
                    .collect::<Option<Vec<_>>>();
                bindings.and_then(|bindings| {
                    if bindings.is_empty() {
                        None
                    } else if bindings
                        .iter()
                        .all(|binding| binding == bindings.first().expect("not empty"))
                    {
                        bindings.first().cloned()
                    } else if are_function_constructor_bindings(&bindings) {
                        Some(LocalFunctionBinding::Builtin(
                            FUNCTION_CONSTRUCTOR_FAMILY_BUILTIN.to_string(),
                        ))
                    } else {
                        None
                    }
                })
            }
            _ => None,
        };
        let mut static_done = None;
        let mut static_value = None;
        let current_index_local = self.allocate_temp_local();
        self.push_local_get(iterator_binding.index_local);
        self.push_local_set(current_index_local);

        match &iterator_binding.source {
            IteratorSourceKind::StaticArray {
                values,
                keys_only,
                length_local,
                runtime_name,
            } => {
                if let Some(current_index) = iterator_binding.static_index {
                    let done = current_index >= values.len();
                    static_done = Some(done);
                    static_value = Some(if done {
                        Expression::Undefined
                    } else if *keys_only {
                        Expression::Number(current_index as f64)
                    } else {
                        values
                            .get(current_index)
                            .cloned()
                            .flatten()
                            .unwrap_or(Expression::Undefined)
                    });
                    iterator_binding.static_index = Some(current_index.saturating_add(1));
                } else {
                    iterator_binding.static_index = None;
                }
                self.push_local_get(current_index_local);
                if let Some(length_local) = length_local {
                    self.push_local_get(*length_local);
                } else {
                    self.push_i32_const(values.len() as i32);
                }
                self.push_binary_op(BinaryOp::GreaterThanOrEqual)
                    .expect("static iterator comparisons are supported");
                self.push_local_set(done_local);

                self.push_local_get(done_local);
                self.instructions.push(0x04);
                self.instructions.push(EMPTY_BLOCK_TYPE);
                self.push_control_frame();
                self.push_i32_const(JS_UNDEFINED_TAG);
                self.push_local_set(value_local);
                self.instructions.push(0x05);
                if *keys_only {
                    self.push_local_get(current_index_local);
                } else if let Some(runtime_name) = runtime_name {
                    if !self
                        .emit_dynamic_runtime_array_slot_read_from_local(
                            runtime_name,
                            current_index_local,
                        )
                        .expect("dynamic runtime array iterator reads are supported")
                    {
                        self.emit_runtime_array_iterator_value_from_local(
                            current_index_local,
                            values,
                        )
                        .expect("static iterator values are supported");
                    }
                } else {
                    self.emit_runtime_array_iterator_value_from_local(current_index_local, &values)
                        .expect("static iterator values are supported");
                }
                self.push_local_set(value_local);
                self.push_local_get(current_index_local);
                self.push_i32_const(1);
                self.instructions.push(0x6a);
                self.push_local_set(iterator_binding.index_local);
                self.instructions.push(0x0b);
                self.pop_control_frame();
            }
            IteratorSourceKind::SimpleGenerator {
                steps,
                completion_effects,
            } => {
                iterator_binding.static_index = None;
                let mut open_frames = 0;
                for (index, step) in steps.iter().enumerate() {
                    self.push_local_get(current_index_local);
                    self.push_i32_const(index as i32);
                    self.push_binary_op(BinaryOp::Equal)
                        .expect("generator iterator comparisons are supported");
                    self.instructions.push(0x04);
                    self.instructions.push(EMPTY_BLOCK_TYPE);
                    self.push_control_frame();
                    open_frames += 1;
                    for effect in &step.effects {
                        self.emit_statement(effect)
                            .expect("simple generator effects should be compilable");
                    }
                    match &step.outcome {
                        SimpleGeneratorStepOutcome::Yield(value) => {
                            self.push_i32_const(0);
                            self.push_local_set(done_local);
                            self.emit_numeric_expression(value)
                                .expect("simple generator yields should be compilable");
                            self.push_local_set(value_local);
                            self.push_i32_const((index + 1) as i32);
                            self.push_local_set(iterator_binding.index_local);
                        }
                        SimpleGeneratorStepOutcome::Throw(value) => {
                            self.push_i32_const(1);
                            self.push_local_set(done_local);
                            self.push_i32_const(JS_UNDEFINED_TAG);
                            self.push_local_set(value_local);
                            self.push_i32_const((steps.len() + 1) as i32);
                            self.push_local_set(iterator_binding.index_local);
                            self.emit_statement(&Statement::Throw(value.clone()))
                                .expect("simple generator throw steps should be compilable");
                        }
                    }
                    self.instructions.push(0x05);
                }

                self.push_local_get(current_index_local);
                self.push_i32_const(steps.len() as i32);
                self.push_binary_op(BinaryOp::Equal)
                    .expect("generator completion comparisons are supported");
                self.instructions.push(0x04);
                self.instructions.push(EMPTY_BLOCK_TYPE);
                self.push_control_frame();
                for effect in completion_effects {
                    self.emit_statement(effect)
                        .expect("simple generator completion effects should be compilable");
                }
                self.push_i32_const(1);
                self.push_local_set(done_local);
                self.push_i32_const(JS_UNDEFINED_TAG);
                self.push_local_set(value_local);
                self.push_i32_const((steps.len() + 1) as i32);
                self.push_local_set(iterator_binding.index_local);
                self.instructions.push(0x05);
                self.push_i32_const(1);
                self.push_local_set(done_local);
                self.push_i32_const(JS_UNDEFINED_TAG);
                self.push_local_set(value_local);
                self.instructions.push(0x0b);
                self.pop_control_frame();

                for _ in 0..open_frames {
                    self.instructions.push(0x0b);
                    self.pop_control_frame();
                }
            }
            IteratorSourceKind::TypedArrayView { name: view_name } => {
                iterator_binding.static_index = None;
                let view_length_local = self
                    .runtime_array_length_locals
                    .get(view_name)
                    .copied()
                    .expect("typed array views should have runtime length locals");
                self.push_local_get(current_index_local);
                self.push_local_get(view_length_local);
                self.push_binary_op(BinaryOp::GreaterThanOrEqual)
                    .expect("typed array iterator comparisons are supported");
                self.push_local_set(done_local);

                self.push_local_get(done_local);
                self.instructions.push(0x04);
                self.instructions.push(EMPTY_BLOCK_TYPE);
                self.push_control_frame();
                self.push_i32_const(JS_UNDEFINED_TAG);
                self.push_local_set(value_local);
                self.instructions.push(0x05);
                self.emit_dynamic_runtime_array_slot_read_from_local(
                    &view_name,
                    current_index_local,
                )
                .expect("typed array iterator reads are supported");
                self.push_local_set(value_local);
                self.push_local_get(current_index_local);
                self.push_i32_const(1);
                self.instructions.push(0x6a);
                self.push_local_set(iterator_binding.index_local);
                self.instructions.push(0x0b);
                self.pop_control_frame();
            }
            IteratorSourceKind::DirectArguments { tracked_prefix_len } => {
                iterator_binding.static_index = None;
                let effective_length_local = self.allocate_temp_local();
                if let Some(actual_argument_count_local) = self.actual_argument_count_local {
                    self.push_local_get(actual_argument_count_local);
                    self.push_i32_const(*tracked_prefix_len as i32);
                    self.push_binary_op(BinaryOp::LessThanOrEqual)
                        .expect("argument count comparisons are supported");
                    self.instructions.push(0x04);
                    self.instructions.push(EMPTY_BLOCK_TYPE);
                    self.push_control_frame();
                    self.push_local_get(actual_argument_count_local);
                    self.push_local_set(effective_length_local);
                    self.instructions.push(0x05);
                    self.push_i32_const(*tracked_prefix_len as i32);
                    self.push_local_set(effective_length_local);
                    self.instructions.push(0x0b);
                    self.pop_control_frame();
                } else {
                    self.push_i32_const(*tracked_prefix_len as i32);
                    self.push_local_set(effective_length_local);
                }

                self.push_local_get(current_index_local);
                self.push_local_get(effective_length_local);
                self.push_binary_op(BinaryOp::GreaterThanOrEqual)
                    .expect("argument iterator comparisons are supported");
                self.push_local_set(done_local);

                self.push_local_get(done_local);
                self.instructions.push(0x04);
                self.instructions.push(EMPTY_BLOCK_TYPE);
                self.push_control_frame();
                self.push_i32_const(JS_UNDEFINED_TAG);
                self.push_local_set(value_local);
                self.instructions.push(0x05);
                self.emit_dynamic_direct_arguments_property_read_from_local(current_index_local)
                    .expect("direct arguments iteration reads are supported");
                self.push_local_set(value_local);
                self.push_local_get(current_index_local);
                self.push_i32_const(1);
                self.instructions.push(0x6a);
                self.push_local_set(iterator_binding.index_local);
                self.instructions.push(0x0b);
                self.pop_control_frame();
            }
        }

        self.local_array_iterator_bindings
            .insert(iterator_binding_name, iterator_binding);
        self.local_iterator_step_bindings.insert(
            name.to_string(),
            IteratorStepBinding::Runtime {
                done_local,
                value_local,
                function_binding,
                static_done,
                static_value,
            },
        );
        self.local_kinds
            .insert(name.to_string(), StaticValueKind::Object);
    }

    pub(in crate::backend::direct_wasm) fn resolve_iterator_step_binding_from_expression(
        &self,
        expression: &Expression,
    ) -> Option<IteratorStepBinding> {
        if let Expression::Identifier(name) = expression {
            if let Some(binding) = self.local_iterator_step_bindings.get(name) {
                return Some(binding.clone());
            }
            if let Some((resolved_name, _)) = self.resolve_current_local_binding(name)
                && let Some(binding) = self.local_iterator_step_bindings.get(&resolved_name)
            {
                return Some(binding.clone());
            }
        }
        let Expression::Identifier(name) = self.resolve_bound_alias_expression(expression)? else {
            return None;
        };
        self.local_iterator_step_bindings.get(&name).cloned()
    }

    pub(in crate::backend::direct_wasm) fn update_local_array_binding(
        &mut self,
        name: &str,
        value: &Expression,
    ) {
        let Some(array_binding) = self.resolve_array_binding_from_expression(value) else {
            self.local_array_bindings.remove(name);
            self.runtime_array_slots.remove(name);
            self.tracked_array_function_values.remove(name);
            return;
        };
        let source_binding_name = if let Expression::Identifier(source_name) = value {
            self.resolve_runtime_array_binding_name(source_name)
        } else {
            None
        };
        let copy_internal_rest_runtime_state = source_binding_name
            .as_ref()
            .is_some_and(|source_name| source_name.starts_with("__ayy_array_rest_"));
        let length_local = if copy_internal_rest_runtime_state {
            self.ensure_runtime_array_length_local(name)
        } else if let Some(source_name) = source_binding_name.as_ref() {
            self.runtime_array_length_locals
                .get(source_name)
                .copied()
                .unwrap_or_else(|| self.ensure_runtime_array_length_local(name))
        } else {
            self.ensure_runtime_array_length_local(name)
        };
        self.runtime_array_length_locals
            .insert(name.to_string(), length_local);
        if copy_internal_rest_runtime_state {
            let source_name = source_binding_name
                .as_ref()
                .expect("rest runtime copy should have a source binding");
            if let Some(source_length_local) =
                self.runtime_array_length_locals.get(source_name).copied()
            {
                self.push_local_get(source_length_local);
            } else {
                self.push_i32_const(array_binding.values.len() as i32);
            }
        } else if let Some(source_length_local) =
            self.runtime_array_length_local_for_expression(value)
        {
            self.push_local_get(source_length_local);
        } else {
            self.push_i32_const(array_binding.values.len() as i32);
        }
        self.push_local_set(length_local);
        if copy_internal_rest_runtime_state {
            let source_name = source_binding_name
                .as_ref()
                .expect("rest runtime copy should have a source binding");
            for index in 0..TRACKED_ARRAY_SLOT_LIMIT {
                let target_slot = self.ensure_runtime_array_slot_entry(name, index);
                if let Some(source_slot) = self.runtime_array_slot(source_name, index) {
                    self.push_local_get(source_slot.value_local);
                    self.push_local_set(target_slot.value_local);
                    self.push_local_get(source_slot.present_local);
                    self.push_local_set(target_slot.present_local);
                } else {
                    self.push_i32_const(JS_UNDEFINED_TAG);
                    self.push_local_set(target_slot.value_local);
                    self.push_i32_const(0);
                    self.push_local_set(target_slot.present_local);
                }
            }
        } else if let Some(source_name) = source_binding_name.as_ref() {
            if let Some(source_slots) = self.runtime_array_slots.get(source_name).cloned() {
                self.runtime_array_slots
                    .insert(name.to_string(), source_slots);
            } else {
                self.ensure_runtime_array_slots_for_binding(name, &array_binding);
            }
        } else {
            self.ensure_runtime_array_slots_for_binding(name, &array_binding);
        }
        self.local_array_bindings
            .insert(name.to_string(), array_binding);
        if let Some(source_name) = source_binding_name.as_ref() {
            if let Some(bindings) = self.tracked_array_function_values.get(source_name).cloned() {
                self.tracked_array_function_values
                    .insert(name.to_string(), bindings);
            } else {
                self.tracked_array_function_values.remove(name);
            }
        } else {
            self.tracked_array_function_values.remove(name);
        }
        self.local_kinds
            .insert(name.to_string(), StaticValueKind::Object);
    }
}
