use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn substitute_simple_generator_statements_with_call_frame_bindings(
        &self,
        statements: &[Statement],
        user_function: &UserFunction,
        mapped_arguments: bool,
        call_argument_values: &mut Vec<Expression>,
        arguments_values: &mut Vec<Expression>,
        this_binding: &Expression,
    ) -> Option<Vec<Statement>> {
        let mut transformed = Vec::with_capacity(statements.len());
        for statement in statements {
            let call_arguments = self.simple_generator_call_arguments(call_argument_values);
            let arguments_binding =
                self.simple_generator_arguments_binding_expression(arguments_values);
            let substituted = match statement {
                Statement::Block { body } => Statement::Block {
                    body: self.substitute_simple_generator_statements_with_call_frame_bindings(
                        body,
                        user_function,
                        mapped_arguments,
                        call_argument_values,
                        arguments_values,
                        this_binding,
                    )?,
                },
                Statement::Assign { name, value } => Statement::Assign {
                    name: name.clone(),
                    value: self.substitute_user_function_call_frame_bindings(
                        value,
                        user_function,
                        &call_arguments,
                        this_binding,
                        &arguments_binding,
                    ),
                },
                Statement::Var { name, value } => Statement::Var {
                    name: name.clone(),
                    value: self.substitute_user_function_call_frame_bindings(
                        value,
                        user_function,
                        &call_arguments,
                        this_binding,
                        &arguments_binding,
                    ),
                },
                Statement::Let {
                    name,
                    mutable,
                    value,
                } => Statement::Let {
                    name: name.clone(),
                    mutable: *mutable,
                    value: self.substitute_user_function_call_frame_bindings(
                        value,
                        user_function,
                        &call_arguments,
                        this_binding,
                        &arguments_binding,
                    ),
                },
                Statement::AssignMember {
                    object,
                    property,
                    value,
                } => Statement::AssignMember {
                    object: self.substitute_user_function_call_frame_bindings(
                        object,
                        user_function,
                        &call_arguments,
                        this_binding,
                        &arguments_binding,
                    ),
                    property: self.substitute_user_function_call_frame_bindings(
                        property,
                        user_function,
                        &call_arguments,
                        this_binding,
                        &arguments_binding,
                    ),
                    value: self.substitute_user_function_call_frame_bindings(
                        value,
                        user_function,
                        &call_arguments,
                        this_binding,
                        &arguments_binding,
                    ),
                },
                Statement::Print { values } => Statement::Print {
                    values: values
                        .iter()
                        .map(|value| {
                            self.substitute_user_function_call_frame_bindings(
                                value,
                                user_function,
                                &call_arguments,
                                this_binding,
                                &arguments_binding,
                            )
                        })
                        .collect(),
                },
                Statement::Expression(expression) => {
                    Statement::Expression(self.substitute_user_function_call_frame_bindings(
                        expression,
                        user_function,
                        &call_arguments,
                        this_binding,
                        &arguments_binding,
                    ))
                }
                Statement::Throw(value) => {
                    Statement::Throw(self.substitute_user_function_call_frame_bindings(
                        value,
                        user_function,
                        &call_arguments,
                        this_binding,
                        &arguments_binding,
                    ))
                }
                Statement::Yield { value } => Statement::Yield {
                    value: self.substitute_user_function_call_frame_bindings(
                        value,
                        user_function,
                        &call_arguments,
                        this_binding,
                        &arguments_binding,
                    ),
                },
                Statement::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    let substituted_condition = self.substitute_user_function_call_frame_bindings(
                        condition,
                        user_function,
                        &call_arguments,
                        this_binding,
                        &arguments_binding,
                    );
                    let branch =
                        if self.resolve_static_if_condition_value(&substituted_condition)? {
                            then_branch
                        } else {
                            else_branch
                        };
                    Statement::Block {
                        body: self
                            .substitute_simple_generator_statements_with_call_frame_bindings(
                                branch,
                                user_function,
                                mapped_arguments,
                                call_argument_values,
                                arguments_values,
                                this_binding,
                            )?,
                    }
                }
                _ => return None,
            };
            self.update_simple_generator_call_frame_state(
                statement,
                &substituted,
                user_function,
                mapped_arguments,
                call_argument_values,
                arguments_values,
            );
            transformed.push(substituted);
        }
        Some(transformed)
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
        let mut call_argument_values = user_function
            .params
            .iter()
            .map(|_| Expression::Undefined)
            .collect::<Vec<_>>();
        let mut arguments_values = Vec::new();
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
        let substituted_body = self
            .substitute_simple_generator_statements_with_call_frame_bindings(
                &function.body,
                user_function,
                function.mapped_arguments && !function.strict,
                &mut call_argument_values,
                &mut arguments_values,
                &analysis_this_binding,
            )?;
        let mut steps = Vec::new();
        let mut effects = Vec::new();
        self.analyze_simple_generator_statements(&substituted_body, &mut steps, &mut effects)?;
        Some((steps, effects))
    }

    pub(in crate::backend::direct_wasm) fn resolve_simple_generator_source(
        &self,
        expression: &Expression,
    ) -> Option<(Vec<SimpleGeneratorStep>, Vec<Statement>)> {
        if let Expression::Call { callee, arguments } = expression
            && let Some(LocalFunctionBinding::User(function_name)) =
                self.resolve_function_binding_from_expression(callee)
            && let Some(user_function) = self.module.user_function_map.get(&function_name)
        {
            if !user_function.is_generator() || user_function.has_parameter_defaults() {
                return None;
            }
            let function = self.resolve_registered_function_declaration(&function_name)?;
            let expanded_arguments = self.expand_call_arguments(arguments);
            let mut call_argument_values = expanded_arguments.clone();
            if call_argument_values.len() < user_function.params.len() {
                call_argument_values.resize(user_function.params.len(), Expression::Undefined);
            }
            let mut arguments_values = expanded_arguments;
            let raw_this_binding = Expression::Undefined;
            let analysis_this_binding =
                if self.should_box_sloppy_function_this(user_function, &raw_this_binding) {
                    Expression::This
                } else {
                    raw_this_binding
                };
            let substituted_body = self
                .substitute_simple_generator_statements_with_call_frame_bindings(
                    &function.body,
                    user_function,
                    function.mapped_arguments && !function.strict,
                    &mut call_argument_values,
                    &mut arguments_values,
                    &analysis_this_binding,
                )?;

            let mut steps = Vec::new();
            let mut effects = Vec::new();
            self.analyze_simple_generator_statements(&substituted_body, &mut steps, &mut effects)?;
            return Some((steps, effects));
        }

        let materialized = self.materialize_static_expression(expression);
        if !static_expression_matches(&materialized, expression) {
            return self.resolve_simple_generator_source(&materialized);
        }

        None
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
        if let Some(binding) = self.resolve_static_user_iterator_binding(expression) {
            return Some(binding);
        }
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

    pub(in crate::backend::direct_wasm) fn resolve_static_user_iterator_binding(
        &self,
        expression: &Expression,
    ) -> Option<ArrayValueBinding> {
        let callee = match expression {
            Expression::Call { callee, .. } | Expression::New { callee, .. } => callee.as_ref(),
            _ => return None,
        };
        let LocalFunctionBinding::User(function_name) =
            self.resolve_function_binding_from_expression(callee)?
        else {
            return None;
        };
        let user_function = self.module.user_function_map.get(&function_name)?;
        let next_binding = user_function
            .returned_member_function_bindings
            .iter()
            .find(|binding| binding.property == "next")?;
        let LocalFunctionBinding::User(next_function_name) = &next_binding.binding else {
            return None;
        };
        let mut property_bindings =
            self.resolve_returned_member_capture_bindings_for_value(expression)?;
        let capture_bindings = property_bindings.remove("next")?;

        let mut bindings = capture_bindings;
        let mut values = Vec::new();
        for _ in 0..256 {
            let (step_result, updated_bindings) =
                self.resolve_bound_snapshot_user_function_result(next_function_name, &bindings)?;
            bindings = updated_bindings;
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
}
