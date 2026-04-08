use super::super::*;

pub(in crate::backend::direct_wasm) trait StaticStatementExecutor {
    type Environment;

    fn evaluate_condition(
        &self,
        condition: &Expression,
        environment: &mut Self::Environment,
    ) -> Option<Expression>;

    fn initialize_binding(
        &self,
        name: &str,
        value: &Expression,
        environment: &mut Self::Environment,
    ) -> Option<()>;

    fn assign_binding(
        &self,
        name: &str,
        value: &Expression,
        environment: &mut Self::Environment,
    ) -> Option<()>;

    fn assign_member_binding(
        &self,
        object: &Expression,
        property: &Expression,
        value: &Expression,
        environment: &mut Self::Environment,
    ) -> Option<()>;

    fn execute_expression_statement(
        &self,
        expression: &Expression,
        environment: &mut Self::Environment,
    ) -> Option<()>;

    fn evaluate_return_value(
        &self,
        expression: &Expression,
        environment: &mut Self::Environment,
    ) -> Option<Expression>;

    fn execute_print_statement(
        &self,
        _values: &[Expression],
        _environment: &mut Self::Environment,
    ) -> Option<()> {
        None
    }

    fn execute_throw_statement(
        &self,
        _expression: &Expression,
        _environment: &mut Self::Environment,
    ) -> Option<StaticStatementControl> {
        None
    }

    fn execute_try_statement(
        &self,
        _body: &[Statement],
        _catch_setup: &[Statement],
        _catch_body: &[Statement],
        _environment: &mut Self::Environment,
    ) -> Option<StaticStatementControl>
    where
        Self::Environment: StaticTransactionalEnvironment,
    {
        None
    }
}

impl<T> StaticStatementExecutor for T
where
    T: StaticExpressionExecutor + ?Sized,
{
    type Environment = T::Environment;

    fn evaluate_condition(
        &self,
        condition: &Expression,
        environment: &mut Self::Environment,
    ) -> Option<Expression> {
        self.evaluate_expression(condition, environment)
    }

    fn initialize_binding(
        &self,
        name: &str,
        value: &Expression,
        environment: &mut Self::Environment,
    ) -> Option<()> {
        let value = self
            .evaluate_expression(value, environment)
            .or_else(|| self.materialize_expression(value, environment))?;
        self.initialize_binding_value(name, value, environment)
    }

    fn assign_binding(
        &self,
        name: &str,
        value: &Expression,
        environment: &mut Self::Environment,
    ) -> Option<()> {
        let value = self
            .evaluate_expression(value, environment)
            .or_else(|| self.materialize_expression(value, environment))?;
        self.assign_binding_value(name, value, environment)
    }

    fn assign_member_binding(
        &self,
        object: &Expression,
        property: &Expression,
        value: &Expression,
        environment: &mut Self::Environment,
    ) -> Option<()> {
        let property = self
            .evaluate_expression(property, environment)
            .or_else(|| self.materialize_expression(property, environment))?;
        let value = self
            .evaluate_expression(value, environment)
            .or_else(|| self.materialize_expression(value, environment))?;
        self.assign_member_binding_value(object, property, value, environment)
    }

    fn execute_expression_statement(
        &self,
        expression: &Expression,
        environment: &mut Self::Environment,
    ) -> Option<()> {
        self.evaluate_expression(expression, environment)?;
        Some(())
    }

    fn evaluate_return_value(
        &self,
        expression: &Expression,
        environment: &mut Self::Environment,
    ) -> Option<Expression> {
        self.evaluate_expression(expression, environment)
    }

    fn execute_print_statement(
        &self,
        values: &[Expression],
        environment: &mut Self::Environment,
    ) -> Option<()> {
        self.execute_print(values, environment)
    }

    fn execute_throw_statement(
        &self,
        expression: &Expression,
        environment: &mut Self::Environment,
    ) -> Option<StaticStatementControl> {
        self.execute_throw(expression, environment)
    }

    fn execute_try_statement(
        &self,
        body: &[Statement],
        catch_setup: &[Statement],
        catch_body: &[Statement],
        environment: &mut Self::Environment,
    ) -> Option<StaticStatementControl>
    where
        Self::Environment: StaticTransactionalEnvironment,
    {
        self.execute_try(body, catch_setup, catch_body, environment)
    }
}
