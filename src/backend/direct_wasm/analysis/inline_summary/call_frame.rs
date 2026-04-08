use super::*;

pub(in crate::backend::direct_wasm) fn expression_mentions_call_frame_state(
    expression: &Expression,
) -> bool {
    match expression {
        Expression::Identifier(name) => {
            name == "arguments"
                || scoped_binding_source_name(name)
                    .is_some_and(|source_name| source_name == "arguments")
        }
        Expression::Member { object, property } => {
            expression_mentions_call_frame_state(object)
                || expression_mentions_call_frame_state(property)
        }
        Expression::Assign { value, .. } => expression_mentions_call_frame_state(value),
        Expression::AssignMember {
            object,
            property,
            value,
        } => {
            expression_mentions_call_frame_state(object)
                || expression_mentions_call_frame_state(property)
                || expression_mentions_call_frame_state(value)
        }
        Expression::AssignSuperMember { property, value } => {
            expression_mentions_call_frame_state(property)
                || expression_mentions_call_frame_state(value)
        }
        Expression::Await(expression)
        | Expression::EnumerateKeys(expression)
        | Expression::GetIterator(expression)
        | Expression::IteratorClose(expression)
        | Expression::Unary { expression, .. } => expression_mentions_call_frame_state(expression),
        Expression::Binary { left, right, .. } => {
            expression_mentions_call_frame_state(left)
                || expression_mentions_call_frame_state(right)
        }
        Expression::Conditional {
            condition,
            then_expression,
            else_expression,
        } => {
            expression_mentions_call_frame_state(condition)
                || expression_mentions_call_frame_state(then_expression)
                || expression_mentions_call_frame_state(else_expression)
        }
        Expression::Sequence(expressions) => {
            expressions.iter().any(expression_mentions_call_frame_state)
        }
        Expression::Call { callee, arguments } | Expression::New { callee, arguments } => {
            matches!(callee.as_ref(), Expression::Identifier(name) if name == "eval")
                || matches!(
                    callee.as_ref(),
                    Expression::Sequence(expressions)
                        if matches!(expressions.last(), Some(Expression::Identifier(name)) if name == "eval")
                )
                || expression_mentions_call_frame_state(callee)
                || arguments.iter().any(|argument| match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                        expression_mentions_call_frame_state(expression)
                    }
                })
        }
        Expression::SuperCall { .. } => true,
        Expression::Array(elements) => elements.iter().any(|element| match element {
            crate::ir::hir::ArrayElement::Expression(expression)
            | crate::ir::hir::ArrayElement::Spread(expression) => {
                expression_mentions_call_frame_state(expression)
            }
        }),
        Expression::Object(entries) => entries.iter().any(|entry| match entry {
            crate::ir::hir::ObjectEntry::Data { key, value } => {
                expression_mentions_call_frame_state(key)
                    || expression_mentions_call_frame_state(value)
            }
            crate::ir::hir::ObjectEntry::Getter { key, getter } => {
                expression_mentions_call_frame_state(key)
                    || expression_mentions_call_frame_state(getter)
            }
            crate::ir::hir::ObjectEntry::Setter { key, setter } => {
                expression_mentions_call_frame_state(key)
                    || expression_mentions_call_frame_state(setter)
            }
            crate::ir::hir::ObjectEntry::Spread(expression) => {
                expression_mentions_call_frame_state(expression)
            }
        }),
        Expression::SuperMember { .. } => false,
        Expression::This | Expression::NewTarget | Expression::Sent => true,
        Expression::Update { .. }
        | Expression::Number(_)
        | Expression::BigInt(_)
        | Expression::String(_)
        | Expression::Bool(_)
        | Expression::Null
        | Expression::Undefined => false,
    }
}

pub(in crate::backend::direct_wasm) fn inline_summary_mentions_call_frame_state(
    summary: &InlineFunctionSummary,
) -> bool {
    summary.effects.iter().any(|effect| match effect {
        InlineFunctionEffect::Assign { value, .. } => expression_mentions_call_frame_state(value),
        InlineFunctionEffect::Update { .. } => false,
        InlineFunctionEffect::Expression(expression) => {
            expression_mentions_call_frame_state(expression)
        }
    }) || summary
        .return_value
        .as_ref()
        .is_some_and(expression_mentions_call_frame_state)
}

pub(in crate::backend::direct_wasm) fn expression_mentions_unsupported_explicit_call_frame_state(
    expression: &Expression,
) -> bool {
    match expression {
        Expression::Identifier(name) => name == "eval",
        Expression::Member { object, property } => {
            expression_mentions_unsupported_explicit_call_frame_state(object)
                || expression_mentions_unsupported_explicit_call_frame_state(property)
        }
        Expression::Assign { value, .. } => {
            expression_mentions_unsupported_explicit_call_frame_state(value)
        }
        Expression::AssignMember {
            object,
            property,
            value,
        } => {
            expression_mentions_unsupported_explicit_call_frame_state(object)
                || expression_mentions_unsupported_explicit_call_frame_state(property)
                || expression_mentions_unsupported_explicit_call_frame_state(value)
        }
        Expression::AssignSuperMember { .. } | Expression::SuperMember { .. } => true,
        Expression::Await(expression)
        | Expression::EnumerateKeys(expression)
        | Expression::GetIterator(expression)
        | Expression::IteratorClose(expression)
        | Expression::Unary { expression, .. } => {
            expression_mentions_unsupported_explicit_call_frame_state(expression)
        }
        Expression::Binary { left, right, .. } => {
            expression_mentions_unsupported_explicit_call_frame_state(left)
                || expression_mentions_unsupported_explicit_call_frame_state(right)
        }
        Expression::Conditional {
            condition,
            then_expression,
            else_expression,
        } => {
            expression_mentions_unsupported_explicit_call_frame_state(condition)
                || expression_mentions_unsupported_explicit_call_frame_state(then_expression)
                || expression_mentions_unsupported_explicit_call_frame_state(else_expression)
        }
        Expression::Sequence(expressions) => expressions
            .iter()
            .any(expression_mentions_unsupported_explicit_call_frame_state),
        Expression::Call { callee, arguments } | Expression::New { callee, arguments } => {
            matches!(callee.as_ref(), Expression::Identifier(name) if name == "eval")
                || matches!(
                    callee.as_ref(),
                    Expression::Sequence(expressions)
                        if matches!(expressions.last(), Some(Expression::Identifier(name)) if name == "eval")
                )
                || expression_mentions_unsupported_explicit_call_frame_state(callee)
                || arguments.iter().any(|argument| match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                        expression_mentions_unsupported_explicit_call_frame_state(expression)
                    }
                })
        }
        Expression::SuperCall { .. } | Expression::NewTarget | Expression::Sent => true,
        Expression::Array(elements) => elements.iter().any(|element| match element {
            crate::ir::hir::ArrayElement::Expression(expression)
            | crate::ir::hir::ArrayElement::Spread(expression) => {
                expression_mentions_unsupported_explicit_call_frame_state(expression)
            }
        }),
        Expression::Object(entries) => entries.iter().any(|entry| match entry {
            crate::ir::hir::ObjectEntry::Data { key, value } => {
                expression_mentions_unsupported_explicit_call_frame_state(key)
                    || expression_mentions_unsupported_explicit_call_frame_state(value)
            }
            crate::ir::hir::ObjectEntry::Getter { key, getter } => {
                expression_mentions_unsupported_explicit_call_frame_state(key)
                    || expression_mentions_unsupported_explicit_call_frame_state(getter)
            }
            crate::ir::hir::ObjectEntry::Setter { key, setter } => {
                expression_mentions_unsupported_explicit_call_frame_state(key)
                    || expression_mentions_unsupported_explicit_call_frame_state(setter)
            }
            crate::ir::hir::ObjectEntry::Spread(expression) => {
                expression_mentions_unsupported_explicit_call_frame_state(expression)
            }
        }),
        Expression::This
        | Expression::Update { .. }
        | Expression::Number(_)
        | Expression::BigInt(_)
        | Expression::String(_)
        | Expression::Bool(_)
        | Expression::Null
        | Expression::Undefined => false,
    }
}

pub(in crate::backend::direct_wasm) fn inline_summary_mentions_unsupported_explicit_call_frame_state(
    summary: &InlineFunctionSummary,
) -> bool {
    summary.effects.iter().any(|effect| match effect {
        InlineFunctionEffect::Assign { value, .. } => {
            expression_mentions_unsupported_explicit_call_frame_state(value)
        }
        InlineFunctionEffect::Update { .. } => false,
        InlineFunctionEffect::Expression(expression) => {
            expression_mentions_unsupported_explicit_call_frame_state(expression)
        }
    }) || summary
        .return_value
        .as_ref()
        .is_some_and(expression_mentions_unsupported_explicit_call_frame_state)
}
