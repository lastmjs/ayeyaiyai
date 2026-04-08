use super::*;

pub(in crate::backend::direct_wasm) fn substitute_self_referential_binding_snapshot(
    expression: &Expression,
    name: &str,
    snapshot: &Expression,
) -> Expression {
    match expression {
        Expression::Identifier(identifier) if identifier == name => snapshot.clone(),
        Expression::Member { object, property } => Expression::Member {
            object: Box::new(substitute_self_referential_binding_snapshot(
                object, name, snapshot,
            )),
            property: Box::new(substitute_self_referential_binding_snapshot(
                property, name, snapshot,
            )),
        },
        Expression::Assign {
            name: target,
            value,
        } => Expression::Assign {
            name: target.clone(),
            value: Box::new(substitute_self_referential_binding_snapshot(
                value, name, snapshot,
            )),
        },
        Expression::AssignMember {
            object,
            property,
            value,
        } => Expression::AssignMember {
            object: Box::new(substitute_self_referential_binding_snapshot(
                object, name, snapshot,
            )),
            property: Box::new(substitute_self_referential_binding_snapshot(
                property, name, snapshot,
            )),
            value: Box::new(substitute_self_referential_binding_snapshot(
                value, name, snapshot,
            )),
        },
        Expression::AssignSuperMember { property, value } => Expression::AssignSuperMember {
            property: Box::new(substitute_self_referential_binding_snapshot(
                property, name, snapshot,
            )),
            value: Box::new(substitute_self_referential_binding_snapshot(
                value, name, snapshot,
            )),
        },
        Expression::Await(value) => Expression::Await(Box::new(
            substitute_self_referential_binding_snapshot(value, name, snapshot),
        )),
        Expression::EnumerateKeys(value) => Expression::EnumerateKeys(Box::new(
            substitute_self_referential_binding_snapshot(value, name, snapshot),
        )),
        Expression::GetIterator(value) => Expression::GetIterator(Box::new(
            substitute_self_referential_binding_snapshot(value, name, snapshot),
        )),
        Expression::IteratorClose(value) => Expression::IteratorClose(Box::new(
            substitute_self_referential_binding_snapshot(value, name, snapshot),
        )),
        Expression::Unary { op, expression } => Expression::Unary {
            op: *op,
            expression: Box::new(substitute_self_referential_binding_snapshot(
                expression, name, snapshot,
            )),
        },
        Expression::Binary { op, left, right } => Expression::Binary {
            op: *op,
            left: Box::new(substitute_self_referential_binding_snapshot(
                left, name, snapshot,
            )),
            right: Box::new(substitute_self_referential_binding_snapshot(
                right, name, snapshot,
            )),
        },
        Expression::Conditional {
            condition,
            then_expression,
            else_expression,
        } => Expression::Conditional {
            condition: Box::new(substitute_self_referential_binding_snapshot(
                condition, name, snapshot,
            )),
            then_expression: Box::new(substitute_self_referential_binding_snapshot(
                then_expression,
                name,
                snapshot,
            )),
            else_expression: Box::new(substitute_self_referential_binding_snapshot(
                else_expression,
                name,
                snapshot,
            )),
        },
        Expression::Sequence(expressions) => Expression::Sequence(
            expressions
                .iter()
                .map(|expression| {
                    substitute_self_referential_binding_snapshot(expression, name, snapshot)
                })
                .collect(),
        ),
        Expression::Call { callee, arguments } => Expression::Call {
            callee: Box::new(substitute_self_referential_binding_snapshot(
                callee, name, snapshot,
            )),
            arguments: arguments
                .iter()
                .map(|argument| match argument {
                    CallArgument::Expression(expression) => CallArgument::Expression(
                        substitute_self_referential_binding_snapshot(expression, name, snapshot),
                    ),
                    CallArgument::Spread(expression) => CallArgument::Spread(
                        substitute_self_referential_binding_snapshot(expression, name, snapshot),
                    ),
                })
                .collect(),
        },
        Expression::SuperCall { callee, arguments } => Expression::SuperCall {
            callee: Box::new(substitute_self_referential_binding_snapshot(
                callee, name, snapshot,
            )),
            arguments: arguments
                .iter()
                .map(|argument| match argument {
                    CallArgument::Expression(expression) => CallArgument::Expression(
                        substitute_self_referential_binding_snapshot(expression, name, snapshot),
                    ),
                    CallArgument::Spread(expression) => CallArgument::Spread(
                        substitute_self_referential_binding_snapshot(expression, name, snapshot),
                    ),
                })
                .collect(),
        },
        Expression::New { callee, arguments } => Expression::New {
            callee: Box::new(substitute_self_referential_binding_snapshot(
                callee, name, snapshot,
            )),
            arguments: arguments
                .iter()
                .map(|argument| match argument {
                    CallArgument::Expression(expression) => CallArgument::Expression(
                        substitute_self_referential_binding_snapshot(expression, name, snapshot),
                    ),
                    CallArgument::Spread(expression) => CallArgument::Spread(
                        substitute_self_referential_binding_snapshot(expression, name, snapshot),
                    ),
                })
                .collect(),
        },
        Expression::Array(elements) => Expression::Array(
            elements
                .iter()
                .map(|element| match element {
                    crate::ir::hir::ArrayElement::Expression(expression) => {
                        crate::ir::hir::ArrayElement::Expression(
                            substitute_self_referential_binding_snapshot(
                                expression, name, snapshot,
                            ),
                        )
                    }
                    crate::ir::hir::ArrayElement::Spread(expression) => {
                        crate::ir::hir::ArrayElement::Spread(
                            substitute_self_referential_binding_snapshot(
                                expression, name, snapshot,
                            ),
                        )
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
                            key: substitute_self_referential_binding_snapshot(key, name, snapshot),
                            value: substitute_self_referential_binding_snapshot(
                                value, name, snapshot,
                            ),
                        }
                    }
                    crate::ir::hir::ObjectEntry::Getter { key, getter } => {
                        crate::ir::hir::ObjectEntry::Getter {
                            key: substitute_self_referential_binding_snapshot(key, name, snapshot),
                            getter: substitute_self_referential_binding_snapshot(
                                getter, name, snapshot,
                            ),
                        }
                    }
                    crate::ir::hir::ObjectEntry::Setter { key, setter } => {
                        crate::ir::hir::ObjectEntry::Setter {
                            key: substitute_self_referential_binding_snapshot(key, name, snapshot),
                            setter: substitute_self_referential_binding_snapshot(
                                setter, name, snapshot,
                            ),
                        }
                    }
                    crate::ir::hir::ObjectEntry::Spread(expression) => {
                        crate::ir::hir::ObjectEntry::Spread(
                            substitute_self_referential_binding_snapshot(
                                expression, name, snapshot,
                            ),
                        )
                    }
                })
                .collect(),
        ),
        Expression::SuperMember { property } => Expression::SuperMember {
            property: Box::new(substitute_self_referential_binding_snapshot(
                property, name, snapshot,
            )),
        },
        _ => expression.clone(),
    }
}
