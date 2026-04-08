use super::super::*;

pub(in crate::backend::direct_wasm) fn rewrite_inline_function_summary_bindings(
    summary: &InlineFunctionSummary,
    bindings: &HashMap<String, Expression>,
) -> InlineFunctionSummary {
    let rewrite_binding_name = |name: &str| {
        bindings
            .get(name)
            .and_then(|binding| match binding {
                Expression::Identifier(identifier) => Some(identifier.clone()),
                _ => None,
            })
            .unwrap_or_else(|| name.to_string())
    };
    InlineFunctionSummary {
        effects: summary
            .effects
            .iter()
            .map(|effect| match effect {
                InlineFunctionEffect::Assign { name, value } => InlineFunctionEffect::Assign {
                    name: rewrite_binding_name(name),
                    value: substitute_inline_summary_bindings(value, bindings),
                },
                InlineFunctionEffect::Update { name, op, prefix } => InlineFunctionEffect::Update {
                    name: rewrite_binding_name(name),
                    op: *op,
                    prefix: *prefix,
                },
                InlineFunctionEffect::Expression(expression) => InlineFunctionEffect::Expression(
                    substitute_inline_summary_bindings(expression, bindings),
                ),
            })
            .collect(),
        return_value: summary
            .return_value
            .as_ref()
            .map(|value| substitute_inline_summary_bindings(value, bindings)),
    }
}

pub(in crate::backend::direct_wasm) fn substitute_inline_summary_bindings(
    expression: &Expression,
    bindings: &HashMap<String, Expression>,
) -> Expression {
    match expression {
        Expression::Identifier(name) => bindings
            .get(name)
            .cloned()
            .unwrap_or_else(|| expression.clone()),
        Expression::Member { object, property } => Expression::Member {
            object: Box::new(substitute_inline_summary_bindings(object, bindings)),
            property: Box::new(substitute_inline_summary_bindings(property, bindings)),
        },
        Expression::SuperMember { property } => Expression::SuperMember {
            property: Box::new(substitute_inline_summary_bindings(property, bindings)),
        },
        Expression::Assign { name, value } => Expression::Assign {
            name: name.clone(),
            value: Box::new(substitute_inline_summary_bindings(value, bindings)),
        },
        Expression::AssignMember {
            object,
            property,
            value,
        } => Expression::AssignMember {
            object: Box::new(substitute_inline_summary_bindings(object, bindings)),
            property: Box::new(substitute_inline_summary_bindings(property, bindings)),
            value: Box::new(substitute_inline_summary_bindings(value, bindings)),
        },
        Expression::AssignSuperMember { property, value } => Expression::AssignSuperMember {
            property: Box::new(substitute_inline_summary_bindings(property, bindings)),
            value: Box::new(substitute_inline_summary_bindings(value, bindings)),
        },
        Expression::Await(value) => Expression::Await(Box::new(
            substitute_inline_summary_bindings(value, bindings),
        )),
        Expression::EnumerateKeys(value) => Expression::EnumerateKeys(Box::new(
            substitute_inline_summary_bindings(value, bindings),
        )),
        Expression::GetIterator(value) => Expression::GetIterator(Box::new(
            substitute_inline_summary_bindings(value, bindings),
        )),
        Expression::IteratorClose(value) => Expression::IteratorClose(Box::new(
            substitute_inline_summary_bindings(value, bindings),
        )),
        Expression::Unary { op, expression } => Expression::Unary {
            op: *op,
            expression: Box::new(substitute_inline_summary_bindings(expression, bindings)),
        },
        Expression::Binary { op, left, right } => Expression::Binary {
            op: *op,
            left: Box::new(substitute_inline_summary_bindings(left, bindings)),
            right: Box::new(substitute_inline_summary_bindings(right, bindings)),
        },
        Expression::Conditional {
            condition,
            then_expression,
            else_expression,
        } => Expression::Conditional {
            condition: Box::new(substitute_inline_summary_bindings(condition, bindings)),
            then_expression: Box::new(substitute_inline_summary_bindings(
                then_expression,
                bindings,
            )),
            else_expression: Box::new(substitute_inline_summary_bindings(
                else_expression,
                bindings,
            )),
        },
        Expression::Sequence(expressions) => Expression::Sequence(
            expressions
                .iter()
                .map(|expression| substitute_inline_summary_bindings(expression, bindings))
                .collect(),
        ),
        Expression::Call { callee, arguments } => Expression::Call {
            callee: Box::new(substitute_inline_summary_bindings(callee, bindings)),
            arguments: arguments
                .iter()
                .map(|argument| match argument {
                    CallArgument::Expression(expression) => CallArgument::Expression(
                        substitute_inline_summary_bindings(expression, bindings),
                    ),
                    CallArgument::Spread(expression) => CallArgument::Spread(
                        substitute_inline_summary_bindings(expression, bindings),
                    ),
                })
                .collect(),
        },
        Expression::SuperCall { callee, arguments } => Expression::SuperCall {
            callee: Box::new(substitute_inline_summary_bindings(callee, bindings)),
            arguments: arguments
                .iter()
                .map(|argument| match argument {
                    CallArgument::Expression(expression) => CallArgument::Expression(
                        substitute_inline_summary_bindings(expression, bindings),
                    ),
                    CallArgument::Spread(expression) => CallArgument::Spread(
                        substitute_inline_summary_bindings(expression, bindings),
                    ),
                })
                .collect(),
        },
        Expression::New { callee, arguments } => Expression::New {
            callee: Box::new(substitute_inline_summary_bindings(callee, bindings)),
            arguments: arguments
                .iter()
                .map(|argument| match argument {
                    CallArgument::Expression(expression) => CallArgument::Expression(
                        substitute_inline_summary_bindings(expression, bindings),
                    ),
                    CallArgument::Spread(expression) => CallArgument::Spread(
                        substitute_inline_summary_bindings(expression, bindings),
                    ),
                })
                .collect(),
        },
        Expression::Update { name, op, prefix } => Expression::Update {
            name: bindings
                .get(name)
                .and_then(|binding| match binding {
                    Expression::Identifier(identifier) => Some(identifier.clone()),
                    _ => None,
                })
                .unwrap_or_else(|| name.clone()),
            op: *op,
            prefix: *prefix,
        },
        Expression::Array(elements) => Expression::Array(
            elements
                .iter()
                .map(|element| match element {
                    crate::ir::hir::ArrayElement::Expression(expression) => {
                        crate::ir::hir::ArrayElement::Expression(
                            substitute_inline_summary_bindings(expression, bindings),
                        )
                    }
                    crate::ir::hir::ArrayElement::Spread(expression) => {
                        crate::ir::hir::ArrayElement::Spread(substitute_inline_summary_bindings(
                            expression, bindings,
                        ))
                    }
                })
                .collect(),
        ),
        Expression::Object(entries) => Expression::Object(
            entries
                .iter()
                .map(|entry| match entry {
                    crate::ir::hir::ObjectEntry::Data { key, value } => {
                        crate::ir::hir::ObjectEntry::Data {
                            key: substitute_inline_summary_bindings(key, bindings),
                            value: substitute_inline_summary_bindings(value, bindings),
                        }
                    }
                    crate::ir::hir::ObjectEntry::Getter { key, getter } => {
                        crate::ir::hir::ObjectEntry::Getter {
                            key: substitute_inline_summary_bindings(key, bindings),
                            getter: substitute_inline_summary_bindings(getter, bindings),
                        }
                    }
                    crate::ir::hir::ObjectEntry::Setter { key, setter } => {
                        crate::ir::hir::ObjectEntry::Setter {
                            key: substitute_inline_summary_bindings(key, bindings),
                            setter: substitute_inline_summary_bindings(setter, bindings),
                        }
                    }
                    crate::ir::hir::ObjectEntry::Spread(expression) => {
                        crate::ir::hir::ObjectEntry::Spread(substitute_inline_summary_bindings(
                            expression, bindings,
                        ))
                    }
                })
                .collect(),
        ),
        _ => expression.clone(),
    }
}
