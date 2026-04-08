use super::*;

#[path = "core_kinds/compound.rs"]
mod compound;
#[path = "core_kinds/primitives.rs"]
mod primitives;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn infer_value_kind(
        &self,
        expression: &Expression,
    ) -> Option<StaticValueKind> {
        match expression {
            Expression::Number(_)
            | Expression::BigInt(_)
            | Expression::String(_)
            | Expression::Bool(_)
            | Expression::Null
            | Expression::Undefined
            | Expression::Identifier(_)
            | Expression::Unary { .. }
            | Expression::Binary { .. }
            | Expression::Conditional { .. } => self.infer_primitive_expression_kind(expression),
            _ => self.infer_compound_expression_kind(expression),
        }
    }
}
