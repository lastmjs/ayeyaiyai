use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_static_string_concat_value(
        &self,
        expression: &Expression,
        current_function_name: Option<&str>,
    ) -> Option<String> {
        let materialized = self.materialize_static_expression(expression);
        if !static_expression_matches(&materialized, expression) {
            return self.resolve_static_string_concat_value(&materialized, current_function_name);
        }
        if let Expression::Binary {
            op: BinaryOp::Add,
            left,
            right,
        } = expression
            && let Some(StaticEvalOutcome::Value(value)) = self
                .resolve_static_addition_outcome_with_context(left, right, current_function_name)
        {
            return self.resolve_static_string_concat_value(&value, current_function_name);
        }
        match expression {
            Expression::Number(value) => {
                if value.is_nan() {
                    Some("NaN".to_string())
                } else if value.is_infinite() {
                    Some(
                        if value.is_sign_positive() {
                            "Infinity"
                        } else {
                            "-Infinity"
                        }
                        .to_string(),
                    )
                } else if *value == 0.0 && value.is_sign_negative() {
                    Some("-0".to_string())
                } else if value.fract() == 0.0 {
                    Some((*value as i64).to_string())
                } else {
                    Some(value.to_string())
                }
            }
            Expression::BigInt(value) => Some(parse_static_bigint_literal(value)?.to_string()),
            Expression::Unary {
                op: UnaryOp::Negate,
                ..
            } => Some(self.resolve_static_bigint_value(expression)?.to_string()),
            Expression::Bool(value) => Some(if *value { "true" } else { "false" }.to_string()),
            Expression::Null => Some("null".to_string()),
            Expression::Undefined => Some("undefined".to_string()),
            Expression::Identifier(name)
                if name == "undefined" && self.is_unshadowed_builtin_identifier(name) =>
            {
                Some("undefined".to_string())
            }
            Expression::Identifier(name)
                if name == "NaN" && self.is_unshadowed_builtin_identifier(name) =>
            {
                Some("NaN".to_string())
            }
            Expression::Unary {
                op: UnaryOp::TypeOf,
                expression,
            } => self
                .infer_typeof_operand_kind(expression)
                .and_then(StaticValueKind::as_typeof_str)
                .map(str::to_string),
            _ => self.resolve_static_string_value_with_context(expression, current_function_name),
        }
    }
}
