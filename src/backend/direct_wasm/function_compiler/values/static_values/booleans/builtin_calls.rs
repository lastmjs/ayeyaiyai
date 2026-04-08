use super::super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_static_array_is_array_call_result(
        &self,
        expression: &Expression,
    ) -> Option<bool> {
        let Expression::Call { callee, arguments } = expression else {
            return None;
        };
        let Expression::Member { object, property } = callee.as_ref() else {
            return None;
        };
        if !matches!(object.as_ref(), Expression::Identifier(name) if name == "Array" && self.is_unshadowed_builtin_identifier(name))
            || !matches!(property.as_ref(), Expression::String(name) if name == "isArray")
        {
            return None;
        }
        let argument = match arguments.first() {
            Some(CallArgument::Expression(expression)) | Some(CallArgument::Spread(expression)) => {
                expression
            }
            None => return Some(false),
        };
        Some(
            !matches!(argument, Expression::Identifier(name) if self.state.speculation.static_semantics.has_local_typed_array_view_binding(name))
                && self
                    .resolve_array_binding_from_expression(argument)
                    .is_some(),
        )
    }

    pub(in crate::backend::direct_wasm) fn resolve_static_is_nan_call_result(
        &self,
        expression: &Expression,
    ) -> Option<bool> {
        let Expression::Call { callee, arguments } = expression else {
            return None;
        };
        if !matches!(callee.as_ref(), Expression::Identifier(name) if name == "isNaN" && self.is_unshadowed_builtin_identifier(name))
        {
            return None;
        }
        let argument = match arguments.first() {
            Some(CallArgument::Expression(expression)) | Some(CallArgument::Spread(expression)) => {
                expression
            }
            None => &Expression::Undefined,
        };
        let resolved_argument = self
            .resolve_static_primitive_expression_with_context(
                argument,
                self.current_function_name(),
            )
            .unwrap_or_else(|| argument.clone());
        if let Some(number) = self.resolve_static_number_value(&resolved_argument) {
            Some(number.is_nan())
        } else if let Some(text) = self.resolve_static_string_value(&resolved_argument) {
            Some(parse_string_to_i32(&text).is_err())
        } else if matches!(
            self.infer_value_kind(&resolved_argument),
            Some(
                StaticValueKind::Object
                    | StaticValueKind::Function
                    | StaticValueKind::Symbol
                    | StaticValueKind::BigInt
            )
        ) {
            Some(true)
        } else {
            None
        }
    }
}
