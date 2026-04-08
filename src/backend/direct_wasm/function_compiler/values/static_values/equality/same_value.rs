use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_static_same_value_result_with_context(
        &self,
        actual: &Expression,
        expected: &Expression,
        current_function_name: Option<&str>,
    ) -> Option<bool> {
        let actual_primitive =
            self.resolve_static_primitive_expression_with_context(actual, current_function_name);
        let expected_primitive =
            self.resolve_static_primitive_expression_with_context(expected, current_function_name);
        let actual_has_primitive = actual_primitive.is_some();
        let expected_has_primitive = expected_primitive.is_some();

        if let (Some(actual_primitive), Some(expected_primitive)) =
            (actual_primitive, expected_primitive)
        {
            return match (actual_primitive, expected_primitive) {
                (Expression::Number(actual), Expression::Number(expected)) => {
                    if actual.is_nan() && expected.is_nan() {
                        Some(true)
                    } else if actual == 0.0 && expected == 0.0 {
                        Some(actual.is_sign_negative() == expected.is_sign_negative())
                    } else {
                        Some(actual == expected)
                    }
                }
                (Expression::BigInt(actual), Expression::BigInt(expected)) => Some(
                    parse_static_bigint_literal(&actual)?
                        == parse_static_bigint_literal(&expected)?,
                ),
                (Expression::String(actual), Expression::String(expected)) => {
                    Some(actual == expected)
                }
                (Expression::Bool(actual), Expression::Bool(expected)) => Some(actual == expected),
                (Expression::Null, Expression::Null)
                | (Expression::Undefined, Expression::Undefined) => Some(true),
                _ => None,
            };
        }

        let actual_materialized = self.materialize_static_expression(actual);
        let expected_materialized = self.materialize_static_expression(expected);

        let actual_is_this = matches!(actual_materialized, Expression::This);
        let expected_is_this = matches!(expected_materialized, Expression::This);
        let has_static_reference_identity = |expression: &Expression| {
            self.resolve_object_binding_from_expression(expression)
                .is_some()
                || self
                    .resolve_array_binding_from_expression(expression)
                    .is_some()
                || self
                    .resolve_user_function_from_expression(expression)
                    .is_some()
        };

        if (actual_is_this && !expected_is_this)
            && has_static_reference_identity(&expected_materialized)
        {
            return Some(false);
        }

        if (expected_is_this && !actual_is_this)
            && has_static_reference_identity(&actual_materialized)
        {
            return Some(false);
        }

        let actual_symbol = self.resolve_symbol_identity_expression(&actual_materialized);
        let expected_symbol = self.resolve_symbol_identity_expression(&expected_materialized);

        if actual_symbol.is_some()
            && expected_symbol.is_none()
            && (expected_has_primitive
                || has_static_reference_identity(&expected_materialized)
                || expected_is_this)
        {
            return Some(false);
        }

        if expected_symbol.is_some()
            && actual_symbol.is_none()
            && (actual_has_primitive
                || has_static_reference_identity(&actual_materialized)
                || actual_is_this)
        {
            return Some(false);
        }

        if let (Some(actual_symbol), Some(expected_symbol)) = (actual_symbol, expected_symbol) {
            return Some(static_expression_matches(&actual_symbol, &expected_symbol));
        }

        if let (Some(actual_key), Some(expected_key)) = (
            self.resolve_static_reference_identity_key(&actual_materialized),
            self.resolve_static_reference_identity_key(&expected_materialized),
        ) {
            return Some(actual_key == expected_key);
        }

        None
    }

    pub(in crate::backend::direct_wasm) fn resolve_static_object_is_call_result(
        &self,
        expression: &Expression,
    ) -> Option<bool> {
        let Expression::Call { callee, arguments } = expression else {
            return None;
        };
        let Expression::Member { object, property } = callee.as_ref() else {
            return None;
        };
        if !matches!(object.as_ref(), Expression::Identifier(name) if name == "Object" && self.is_unshadowed_builtin_identifier(name))
        {
            return None;
        }
        if !matches!(property.as_ref(), Expression::String(name) if name == "is") {
            return None;
        }
        let [
            CallArgument::Expression(actual),
            CallArgument::Expression(expected),
            ..,
        ] = arguments.as_slice()
        else {
            return None;
        };
        self.resolve_static_same_value_result_with_context(
            actual,
            expected,
            self.current_function_name(),
        )
    }
}
