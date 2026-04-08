use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_static_math_extremum(
        &self,
        arguments: &[CallArgument],
        current_function_name: Option<&str>,
        is_max: bool,
    ) -> Option<f64> {
        let mut result = if is_max {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        };
        for argument in arguments {
            let value =
                self.resolve_static_builtin_math_argument_number(argument, current_function_name)?;
            if value.is_nan() {
                return Some(f64::NAN);
            }
            let replace = if is_max {
                value > result || (value == 0.0 && result == 0.0 && value.is_sign_positive())
            } else {
                value < result || (value == 0.0 && result == 0.0 && value.is_sign_negative())
            };
            if replace {
                result = value;
            }
        }
        Some(result)
    }

    pub(in crate::backend::direct_wasm) fn resolve_static_builtin_math_argument_number(
        &self,
        argument: &CallArgument,
        current_function_name: Option<&str>,
    ) -> Option<f64> {
        let expression = match argument {
            CallArgument::Expression(expression) | CallArgument::Spread(expression) => expression,
        };
        let primitive = self
            .resolve_static_primitive_expression_with_context(expression, current_function_name)
            .unwrap_or_else(|| self.materialize_static_expression(expression));
        if let Some(number) = self.resolve_static_number_value(&primitive) {
            return Some(number);
        }
        match self.infer_value_kind(&primitive) {
            Some(StaticValueKind::Object)
            | Some(StaticValueKind::Function)
            | Some(StaticValueKind::Symbol)
            | Some(StaticValueKind::BigInt) => Some(f64::NAN),
            _ => None,
        }
    }
}
