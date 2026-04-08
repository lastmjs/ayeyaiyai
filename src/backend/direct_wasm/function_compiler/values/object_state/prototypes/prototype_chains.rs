use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn prototype_member_expression(name: &str) -> Expression {
        Expression::Member {
            object: Box::new(Expression::Identifier(name.to_string())),
            property: Box::new(Expression::String("prototype".to_string())),
        }
    }

    pub(in crate::backend::direct_wasm) fn builtin_constructor_object_prototype_expression(
        name: &str,
    ) -> Option<Expression> {
        if matches!(
            name,
            "AggregateError"
                | "EvalError"
                | "RangeError"
                | "ReferenceError"
                | "SyntaxError"
                | "TypeError"
                | "URIError"
        ) {
            return Some(Expression::Identifier("Error".to_string()));
        }
        if builtin_identifier_kind(name) == Some(StaticValueKind::Function)
            || infer_call_result_kind(name).is_some()
        {
            return Some(Self::prototype_member_expression("Function"));
        }
        None
    }

    pub(in crate::backend::direct_wasm) fn builtin_prototype_object_prototype_expression(
        name: &str,
    ) -> Option<Expression> {
        if name == "Object" {
            return Some(Expression::Null);
        }
        if matches!(
            name,
            "AggregateError"
                | "EvalError"
                | "RangeError"
                | "ReferenceError"
                | "SyntaxError"
                | "TypeError"
                | "URIError"
        ) {
            return Some(Self::prototype_member_expression("Error"));
        }
        if name == "Error"
            || builtin_identifier_kind(name) == Some(StaticValueKind::Function)
            || infer_call_result_kind(name).is_some()
        {
            return Some(Self::prototype_member_expression("Object"));
        }
        None
    }
}
