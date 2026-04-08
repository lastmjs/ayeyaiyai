use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn substitute_statement_bindings(
        &self,
        statement: &Statement,
        bindings: &HashMap<String, Expression>,
    ) -> Statement {
        let substitute_name = |name: &str| {
            bindings
                .get(name)
                .and_then(|value| match value {
                    Expression::Identifier(replacement) => Some(replacement.clone()),
                    _ => None,
                })
                .unwrap_or_else(|| name.to_string())
        };

        match statement {
            Statement::Block { body } => Statement::Block {
                body: body
                    .iter()
                    .map(|statement| self.substitute_statement_bindings(statement, bindings))
                    .collect(),
            },
            Statement::Var { name, value } => Statement::Var {
                name: substitute_name(name),
                value: self.substitute_expression_bindings(value, bindings),
            },
            Statement::Let {
                name,
                mutable,
                value,
            } => Statement::Let {
                name: substitute_name(name),
                mutable: *mutable,
                value: self.substitute_expression_bindings(value, bindings),
            },
            Statement::Assign { name, value } => Statement::Assign {
                name: substitute_name(name),
                value: self.substitute_expression_bindings(value, bindings),
            },
            Statement::AssignMember {
                object,
                property,
                value,
            } => Statement::AssignMember {
                object: self.substitute_expression_bindings(object, bindings),
                property: self.substitute_expression_bindings(property, bindings),
                value: self.substitute_expression_bindings(value, bindings),
            },
            Statement::Print { values } => Statement::Print {
                values: values
                    .iter()
                    .map(|value| self.substitute_expression_bindings(value, bindings))
                    .collect(),
            },
            Statement::Expression(expression) => {
                Statement::Expression(self.substitute_expression_bindings(expression, bindings))
            }
            Statement::Throw(value) => {
                Statement::Throw(self.substitute_expression_bindings(value, bindings))
            }
            Statement::Return(value) => {
                Statement::Return(self.substitute_expression_bindings(value, bindings))
            }
            Statement::Yield { value } => Statement::Yield {
                value: self.substitute_expression_bindings(value, bindings),
            },
            Statement::YieldDelegate { value } => Statement::YieldDelegate {
                value: self.substitute_expression_bindings(value, bindings),
            },
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => Statement::If {
                condition: self.substitute_expression_bindings(condition, bindings),
                then_branch: then_branch
                    .iter()
                    .map(|statement| self.substitute_statement_bindings(statement, bindings))
                    .collect(),
                else_branch: else_branch
                    .iter()
                    .map(|statement| self.substitute_statement_bindings(statement, bindings))
                    .collect(),
            },
            _ => statement.clone(),
        }
    }

    pub(in crate::backend::direct_wasm) fn substitute_async_yield_delegate_generator_plan_scope_bindings(
        &self,
        plan: &AsyncYieldDelegateGeneratorPlan,
        bindings: &HashMap<String, Expression>,
    ) -> AsyncYieldDelegateGeneratorPlan {
        AsyncYieldDelegateGeneratorPlan {
            function_name: plan.function_name.clone(),
            prefix_effects: plan
                .prefix_effects
                .iter()
                .map(|statement| self.substitute_statement_bindings(statement, bindings))
                .collect(),
            delegate_expression: self
                .substitute_expression_bindings(&plan.delegate_expression, bindings),
            completion_effects: plan
                .completion_effects
                .iter()
                .map(|statement| self.substitute_statement_bindings(statement, bindings))
                .collect(),
            completion_value: self.substitute_expression_bindings(&plan.completion_value, bindings),
            completion_throw_value: plan
                .completion_throw_value
                .as_ref()
                .map(|value| self.substitute_expression_bindings(value, bindings)),
            scope_bindings: Vec::new(),
        }
    }
}
