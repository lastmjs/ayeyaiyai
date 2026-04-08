use crate::backend::direct_wasm::{
    Expression, evaluate_shared_static_expression, evaluate_static_binary_expression,
};

use super::{StaticExpressionHooks, StaticExpressionMaterialization};
use crate::backend::direct_wasm::StaticExpressionExecutionSource;

pub(in crate::backend::direct_wasm) trait StaticExpressionEvaluation:
    StaticExpressionHooks + StaticExpressionMaterialization
{
    fn evaluate_expression(
        &self,
        expression: &Expression,
        environment: &mut Self::Environment,
    ) -> Option<Expression>;

    fn evaluate_fallback_expression(
        &self,
        expression: &Expression,
        environment: &mut Self::Environment,
    ) -> Option<Expression>;
}

impl<T> StaticExpressionEvaluation for T
where
    T: StaticExpressionExecutionSource + ?Sized,
{
    fn evaluate_expression(
        &self,
        expression: &Expression,
        environment: &mut Self::Environment,
    ) -> Option<Expression> {
        evaluate_shared_static_expression(self, expression, environment)
            .or_else(|| self.evaluate_fallback_expression(expression, environment))
    }

    fn evaluate_fallback_expression(
        &self,
        expression: &Expression,
        environment: &mut Self::Environment,
    ) -> Option<Expression> {
        self.evaluate_special_expression(expression, environment)
            .or_else(|| evaluate_static_binary_expression(self, expression, environment))
            .or_else(|| self.materialize_expression(expression, environment))
    }
}
