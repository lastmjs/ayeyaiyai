use super::super::*;

pub(in crate::backend::direct_wasm) fn inline_summary_side_effect_free_expression(
    expression: &Expression,
) -> bool {
    match expression {
        Expression::Number(_)
        | Expression::BigInt(_)
        | Expression::String(_)
        | Expression::Bool(_)
        | Expression::Null
        | Expression::Undefined
        | Expression::Identifier(_)
        | Expression::This
        | Expression::NewTarget
        | Expression::Sent => true,
        Expression::Member { object, property } => {
            inline_summary_side_effect_free_expression(object)
                && inline_summary_side_effect_free_expression(property)
        }
        Expression::SuperMember { property } => {
            inline_summary_side_effect_free_expression(property)
        }
        Expression::Unary { expression, .. }
        | Expression::Await(expression)
        | Expression::EnumerateKeys(expression)
        | Expression::GetIterator(expression)
        | Expression::IteratorClose(expression) => {
            inline_summary_side_effect_free_expression(expression)
        }
        Expression::Binary { left, right, .. } => {
            inline_summary_side_effect_free_expression(left)
                && inline_summary_side_effect_free_expression(right)
        }
        Expression::Conditional {
            condition,
            then_expression,
            else_expression,
        } => {
            inline_summary_side_effect_free_expression(condition)
                && inline_summary_side_effect_free_expression(then_expression)
                && inline_summary_side_effect_free_expression(else_expression)
        }
        Expression::Sequence(expressions) => expressions
            .iter()
            .all(inline_summary_side_effect_free_expression),
        Expression::Array(elements) => elements.iter().all(|element| match element {
            crate::ir::hir::ArrayElement::Expression(expression)
            | crate::ir::hir::ArrayElement::Spread(expression) => {
                inline_summary_side_effect_free_expression(expression)
            }
        }),
        Expression::Object(entries) => entries.iter().all(|entry| match entry {
            crate::ir::hir::ObjectEntry::Data { key, value } => {
                inline_summary_side_effect_free_expression(key)
                    && inline_summary_side_effect_free_expression(value)
            }
            crate::ir::hir::ObjectEntry::Getter { key, getter } => {
                inline_summary_side_effect_free_expression(key)
                    && inline_summary_side_effect_free_expression(getter)
            }
            crate::ir::hir::ObjectEntry::Setter { key, setter } => {
                inline_summary_side_effect_free_expression(key)
                    && inline_summary_side_effect_free_expression(setter)
            }
            crate::ir::hir::ObjectEntry::Spread(expression) => {
                inline_summary_side_effect_free_expression(expression)
            }
        }),
        Expression::Assign { .. }
        | Expression::AssignMember { .. }
        | Expression::AssignSuperMember { .. }
        | Expression::Call { .. }
        | Expression::SuperCall { .. }
        | Expression::New { .. }
        | Expression::Update { .. } => false,
    }
}
