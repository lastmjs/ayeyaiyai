use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_static_bigint_value(
        &self,
        expression: &Expression,
    ) -> Option<StaticBigInt> {
        let materialized = self.materialize_static_expression(expression);
        if !static_expression_matches(&materialized, expression) {
            return self.resolve_static_bigint_value(&materialized);
        }
        match expression {
            Expression::BigInt(value) => parse_static_bigint_literal(value),
            Expression::Unary {
                op: UnaryOp::Negate,
                expression,
            } => Some(-self.resolve_static_bigint_value(expression)?),
            _ => None,
        }
    }
}
