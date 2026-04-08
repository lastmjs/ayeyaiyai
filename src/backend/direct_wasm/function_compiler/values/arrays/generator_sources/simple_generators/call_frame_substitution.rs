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
                Statement::Return(value) => {
                    Statement::Return(self.substitute_user_function_call_frame_bindings(
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
                Statement::YieldDelegate { value } => Statement::YieldDelegate {
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
                    if let Some(condition_value) =
                        self.resolve_static_if_condition_value(&substituted_condition)
                    {
                        let branch = if condition_value {
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
                    } else {
                        if Self::statement_contains_generator_yield(&Statement::Block {
                            body: then_branch.clone(),
                        }) || Self::statement_contains_generator_yield(&Statement::Block {
                            body: else_branch.clone(),
                        }) {
                            return None;
                        }
                        Statement::If {
                            condition: substituted_condition,
                            then_branch: self
                                .substitute_simple_generator_statements_with_call_frame_bindings(
                                    then_branch,
                                    user_function,
                                    mapped_arguments,
                                    call_argument_values,
                                    arguments_values,
                                    this_binding,
                                )?,
                            else_branch: self
                                .substitute_simple_generator_statements_with_call_frame_bindings(
                                    else_branch,
                                    user_function,
                                    mapped_arguments,
                                    call_argument_values,
                                    arguments_values,
                                    this_binding,
                                )?,
                        }
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

    pub(super) fn split_simple_generator_completion(
        &self,
        mut statements: Vec<Statement>,
    ) -> Option<(Vec<Statement>, Expression)> {
        let completion_value = if let Some(Statement::Return(value)) = statements.last() {
            let value = value.clone();
            statements.pop();
            value
        } else {
            Expression::Undefined
        };
        if statements
            .iter()
            .any(|statement| matches!(statement, Statement::Return(_)))
        {
            return None;
        }
        Some((statements, completion_value))
    }
}
