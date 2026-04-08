use super::*;

pub(in crate::backend::direct_wasm) fn expression_mentions_assertion_builtin(
    expression: &Expression,
) -> bool {
    match expression {
        Expression::Identifier(name) => matches!(
            name.as_str(),
            "__assert" | "__assertSameValue" | "__assertNotSameValue" | "__ayyAssertThrows"
        ),
        Expression::Member { object, property } => {
            (matches!(object.as_ref(), Expression::Identifier(name) if name == "assert")
                && matches!(
                    property.as_ref(),
                    Expression::String(name)
                        if matches!(name.as_str(), "sameValue" | "notSameValue")
                ))
                || expression_mentions_assertion_builtin(object)
                || expression_mentions_assertion_builtin(property)
        }
        Expression::SuperMember { property } => expression_mentions_assertion_builtin(property),
        Expression::Assign { value, .. }
        | Expression::Await(value)
        | Expression::EnumerateKeys(value)
        | Expression::GetIterator(value)
        | Expression::IteratorClose(value)
        | Expression::Unary {
            expression: value, ..
        } => expression_mentions_assertion_builtin(value),
        Expression::AssignMember {
            object,
            property,
            value,
        } => {
            expression_mentions_assertion_builtin(object)
                || expression_mentions_assertion_builtin(property)
                || expression_mentions_assertion_builtin(value)
        }
        Expression::AssignSuperMember { property, value } => {
            expression_mentions_assertion_builtin(property)
                || expression_mentions_assertion_builtin(value)
        }
        Expression::Binary { left, right, .. } => {
            expression_mentions_assertion_builtin(left)
                || expression_mentions_assertion_builtin(right)
        }
        Expression::Conditional {
            condition,
            then_expression,
            else_expression,
        } => {
            expression_mentions_assertion_builtin(condition)
                || expression_mentions_assertion_builtin(then_expression)
                || expression_mentions_assertion_builtin(else_expression)
        }
        Expression::Sequence(expressions) => expressions
            .iter()
            .any(expression_mentions_assertion_builtin),
        Expression::Call { callee, arguments }
        | Expression::SuperCall { callee, arguments }
        | Expression::New { callee, arguments } => {
            expression_mentions_assertion_builtin(callee)
                || arguments.iter().any(|argument| match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                        expression_mentions_assertion_builtin(expression)
                    }
                })
        }
        Expression::Array(elements) => elements.iter().any(|element| match element {
            ArrayElement::Expression(expression) | ArrayElement::Spread(expression) => {
                expression_mentions_assertion_builtin(expression)
            }
        }),
        Expression::Object(entries) => entries.iter().any(|entry| match entry {
            ObjectEntry::Data { key, value } => {
                expression_mentions_assertion_builtin(key)
                    || expression_mentions_assertion_builtin(value)
            }
            ObjectEntry::Getter { key, getter } => {
                expression_mentions_assertion_builtin(key)
                    || expression_mentions_assertion_builtin(getter)
            }
            ObjectEntry::Setter { key, setter } => {
                expression_mentions_assertion_builtin(key)
                    || expression_mentions_assertion_builtin(setter)
            }
            ObjectEntry::Spread(expression) => expression_mentions_assertion_builtin(expression),
        }),
        Expression::Update { .. }
        | Expression::Number(_)
        | Expression::BigInt(_)
        | Expression::String(_)
        | Expression::Bool(_)
        | Expression::Null
        | Expression::Undefined
        | Expression::NewTarget
        | Expression::This
        | Expression::Sent => false,
    }
}

pub(in crate::backend::direct_wasm) fn inline_summary_mentions_assertion_builtin(
    summary: &InlineFunctionSummary,
) -> bool {
    summary.effects.iter().any(|effect| match effect {
        InlineFunctionEffect::Assign { value, .. } => expression_mentions_assertion_builtin(value),
        InlineFunctionEffect::Update { .. } => false,
        InlineFunctionEffect::Expression(expression) => {
            expression_mentions_assertion_builtin(expression)
        }
    }) || summary
        .return_value
        .as_ref()
        .is_some_and(expression_mentions_assertion_builtin)
}
