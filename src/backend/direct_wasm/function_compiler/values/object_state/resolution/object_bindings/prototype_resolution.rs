use super::*;

#[path = "prototype_resolution/prototype_lookup.rs"]
mod prototype_lookup;
#[path = "prototype_resolution/weakref_lookup.rs"]
mod weakref_lookup;

impl<'a> FunctionCompiler<'a> {
    fn normalize_static_object_prototype_target_expression(expression: &Expression) -> Expression {
        match expression {
            Expression::Sequence(expressions) => expressions
                .last()
                .map(Self::normalize_static_object_prototype_target_expression)
                .unwrap_or(Expression::Undefined),
            Expression::Member { object, property } if matches!(property.as_ref(), Expression::String(name) if name == "prototype") => {
                Expression::Member {
                    object: Box::new(Self::normalize_static_object_prototype_target_expression(
                        object,
                    )),
                    property: property.clone(),
                }
            }
            _ => expression.clone(),
        }
    }
}
