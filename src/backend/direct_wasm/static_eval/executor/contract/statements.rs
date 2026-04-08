use super::super::super::execution::{
    execute_static_statement_value, execute_static_try_statement,
};
use crate::backend::direct_wasm::{Expression, Statement, StaticStatementControl};

use super::{StaticBindingMutationExecutor, StaticExpressionEvaluation};

pub(in crate::backend::direct_wasm) trait StaticStatementExecutionExecutor:
    StaticBindingMutationExecutor + StaticExpressionEvaluation
{
    fn execute_print(
        &self,
        values: &[Expression],
        environment: &mut Self::Environment,
    ) -> Option<()> {
        for value in values {
            self.evaluate_expression(value, environment)?;
        }
        Some(())
    }

    fn execute_throw(
        &self,
        expression: &Expression,
        environment: &mut Self::Environment,
    ) -> Option<StaticStatementControl> {
        self.evaluate_expression(expression, environment)?;
        Some(StaticStatementControl::Return(Expression::Undefined))
    }

    fn execute_try(
        &self,
        body: &[Statement],
        catch_setup: &[Statement],
        catch_body: &[Statement],
        environment: &mut Self::Environment,
    ) -> Option<StaticStatementControl> {
        execute_static_try_statement(self, body, catch_setup, catch_body, environment)
    }

    fn execute_static_statements_with_state(
        &self,
        statements: &[Statement],
        environment: &mut Self::Environment,
    ) -> Option<Option<Expression>> {
        execute_static_statement_value(self, statements, environment)
    }

    fn materialize_static_expression_with_state(
        &self,
        expression: &Expression,
        environment: &Self::Environment,
    ) -> Option<Expression> {
        self.materialize_expression_in_forked_environment(expression, environment)
    }

    fn evaluate_static_expression_with_state(
        &self,
        expression: &Expression,
        environment: &mut Self::Environment,
    ) -> Option<Expression> {
        self.evaluate_expression(expression, environment)
    }
}

impl<T> StaticStatementExecutionExecutor for T where
    T: StaticBindingMutationExecutor + StaticExpressionEvaluation + ?Sized
{
}
