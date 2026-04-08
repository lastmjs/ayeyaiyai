use super::*;

pub(in crate::backend::direct_wasm) trait StaticUserFunctionBindingExecutor:
    StaticExpressionExecutor
{
    fn resolve_static_user_function_declaration(
        &self,
        function_name: &str,
    ) -> Option<&FunctionDeclaration>;

    fn resolve_static_user_function_body(&self, function_name: &str) -> Option<&[Statement]> {
        Some(
            &self
                .resolve_static_user_function_declaration(function_name)?
                .body,
        )
    }

    fn resolve_static_user_function_metadata(&self, function_name: &str) -> Option<&UserFunction>;

    fn substitute_static_user_function_argument_bindings(
        &self,
        expression: &Expression,
        user_function: &UserFunction,
        arguments: &[CallArgument],
    ) -> Expression;

    fn materialize_inline_static_user_function_return(
        &self,
        expression: &Expression,
        environment: &mut Self::Environment,
    ) -> Option<Expression>
    where
        Self::Environment: StaticTransactionalEnvironment,
    {
        let mut environment = environment.fork_environment();
        self.materialize_expression(expression, &mut environment)
    }

    fn inline_static_user_function_binding(
        &self,
        function_name: &str,
        arguments: &[CallArgument],
        environment: &mut Self::Environment,
    ) -> Option<Expression>
    where
        Self::Environment: StaticTransactionalEnvironment,
    {
        let user_function = self.resolve_static_user_function_metadata(function_name)?;
        let summary = user_function.inline_summary.as_ref()?;
        if !summary.effects.is_empty() {
            return None;
        }
        let return_value = summary.return_value.as_ref()?;
        let substituted = self.substitute_static_user_function_argument_bindings(
            return_value,
            user_function,
            arguments,
        );
        self.materialize_inline_static_user_function_return(&substituted, environment)
    }
}

pub(in crate::backend::direct_wasm) trait StaticUserFunctionBindingSource {
    fn static_user_function_declaration(&self, function_name: &str)
    -> Option<&FunctionDeclaration>;

    fn static_user_function_metadata(&self, function_name: &str) -> Option<&UserFunction>;

    fn substitute_static_user_function_arguments(
        &self,
        expression: &Expression,
        user_function: &UserFunction,
        arguments: &[CallArgument],
    ) -> Expression;
}

impl<T> StaticUserFunctionBindingExecutor for T
where
    T: StaticExpressionExecutor + StaticUserFunctionBindingSource + ?Sized,
{
    fn resolve_static_user_function_declaration(
        &self,
        function_name: &str,
    ) -> Option<&FunctionDeclaration> {
        self.static_user_function_declaration(function_name)
    }

    fn resolve_static_user_function_metadata(&self, function_name: &str) -> Option<&UserFunction> {
        self.static_user_function_metadata(function_name)
    }

    fn substitute_static_user_function_argument_bindings(
        &self,
        expression: &Expression,
        user_function: &UserFunction,
        arguments: &[CallArgument],
    ) -> Expression {
        self.substitute_static_user_function_arguments(expression, user_function, arguments)
    }
}

pub(in crate::backend::direct_wasm) fn execute_static_function_body<Executor, Environment>(
    executor: &Executor,
    statements: &[Statement],
    environment: &mut Environment,
) -> Option<Expression>
where
    Executor: StaticStatementExecutor<Environment = Environment> + ?Sized,
    Environment: StaticFunctionExecutionEnvironment + StaticTransactionalEnvironment,
{
    environment.clear_function_locals();
    execute_static_statement_value(executor, statements, environment)
        .map(|result| result.unwrap_or(Expression::Undefined))
}

pub(in crate::backend::direct_wasm) fn execute_static_function_body_in_environment<
    Executor,
    Environment,
>(
    executor: &Executor,
    statements: &[Statement],
    environment: &mut Environment,
    effect_mode: StaticFunctionEffectMode,
) -> Option<Expression>
where
    Executor: StaticStatementExecutor<Environment = Environment> + ?Sized,
    Environment: StaticTransactionalEnvironment,
{
    let mut function_environment = environment.fork_environment();
    let result = execute_static_function_body(executor, statements, &mut function_environment)?;
    if matches!(effect_mode, StaticFunctionEffectMode::Commit) {
        environment.commit_environment(function_environment);
    }
    Some(result)
}

pub(in crate::backend::direct_wasm) fn execute_static_user_function_binding_in_environment<
    Executor,
>(
    executor: &Executor,
    binding: &LocalFunctionBinding,
    arguments: &[CallArgument],
    environment: &mut Executor::Environment,
    effect_mode: StaticFunctionEffectMode,
) -> Option<Expression>
where
    Executor: StaticUserFunctionBindingExecutor + ?Sized,
    Executor::Environment: StaticTransactionalEnvironment,
{
    let LocalFunctionBinding::User(function_name) = binding else {
        return None;
    };
    if let Some(result) =
        executor.inline_static_user_function_binding(function_name, arguments, environment)
    {
        return Some(result);
    }
    let statements = executor.resolve_static_user_function_body(function_name)?;
    execute_static_function_body_in_environment(executor, statements, environment, effect_mode)
}
