use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn analyze_simple_generator_statements(
        &self,
        statements: &[Statement],
        async_generator: bool,
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
                Statement::YieldDelegate { value } => {
                    let (mut delegate_steps, mut delegate_completion_effects) =
                        self.resolve_simple_yield_delegate_source(value, async_generator)?;
                    let delegate_ends_in_throw = delegate_steps.last().is_some_and(|step| {
                        matches!(step.outcome, SimpleGeneratorStepOutcome::Throw(_))
                    });
                    if let Some(first_step) = delegate_steps.first_mut() {
                        let mut prefix_effects = std::mem::take(effects);
                        prefix_effects.append(&mut first_step.effects);
                        first_step.effects = prefix_effects;
                    }
                    steps.extend(delegate_steps);
                    effects.append(&mut delegate_completion_effects);
                    if delegate_ends_in_throw {
                        return Some(());
                    }
                }
                Statement::Throw(value) => {
                    steps.push(SimpleGeneratorStep {
                        effects: std::mem::take(effects),
                        outcome: SimpleGeneratorStepOutcome::Throw(value.clone()),
                    });
                    return Some(());
                }
                Statement::Block { body } => {
                    self.analyze_simple_generator_statements(
                        body,
                        async_generator,
                        steps,
                        effects,
                    )?;
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
                    if let Some(condition_value) =
                        self.resolve_static_if_condition_value(&materialized_condition)
                    {
                        let branch = if condition_value {
                            then_branch
                        } else {
                            else_branch
                        };
                        self.analyze_simple_generator_statements(
                            branch,
                            async_generator,
                            steps,
                            effects,
                        )?;
                    } else {
                        if then_branch
                            .iter()
                            .any(Self::statement_contains_generator_yield)
                            || else_branch
                                .iter()
                                .any(Self::statement_contains_generator_yield)
                        {
                            return None;
                        }
                        effects.push(statement.clone());
                    }
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
