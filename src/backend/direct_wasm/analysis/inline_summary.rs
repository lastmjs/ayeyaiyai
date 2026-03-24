use super::*;

pub(in crate::backend::direct_wasm) fn collect_inline_function_summary(
    function: &FunctionDeclaration,
) -> Option<InlineFunctionSummary> {
    let mut summary = InlineFunctionSummary::default();
    let parameter_names = function
        .params
        .iter()
        .map(|param| param.name.clone())
        .collect::<HashSet<_>>();
    let mut local_bindings = HashMap::new();
    for statement in &function.body {
        match statement {
            Statement::Var { name, value } | Statement::Let { name, value, .. } => {
                if parameter_names.contains(name) {
                    return None;
                }
                local_bindings.insert(
                    name.clone(),
                    substitute_inline_summary_bindings(value, &local_bindings),
                );
            }
            Statement::Assign { name, value } => {
                if parameter_names.contains(name) {
                    return None;
                }
                if local_bindings.contains_key(name) {
                    return None;
                }
                summary.effects.push(InlineFunctionEffect::Assign {
                    name: name.clone(),
                    value: substitute_inline_summary_bindings(value, &local_bindings),
                });
            }
            Statement::AssignMember {
                object,
                property,
                value,
            } => {
                let object = substitute_inline_summary_bindings(object, &local_bindings);
                let property = substitute_inline_summary_bindings(property, &local_bindings);
                let value = substitute_inline_summary_bindings(value, &local_bindings);
                if !function.mapped_arguments
                    && matches!(&object, Expression::Identifier(name) if name == "arguments")
                    && inline_summary_side_effect_free_expression(&property)
                    && inline_summary_side_effect_free_expression(&value)
                {
                    continue;
                }
                summary
                    .effects
                    .push(InlineFunctionEffect::Expression(Expression::AssignMember {
                        object: Box::new(object),
                        property: Box::new(property),
                        value: Box::new(value),
                    }));
            }
            Statement::Expression(Expression::Update { name, op, prefix }) => {
                if function.params.iter().any(|param| param.name == *name)
                    || local_bindings.contains_key(name)
                {
                    return None;
                }
                summary.effects.push(InlineFunctionEffect::Update {
                    name: name.clone(),
                    op: *op,
                    prefix: *prefix,
                });
            }
            Statement::Expression(expression) => {
                summary.effects.push(InlineFunctionEffect::Expression(
                    substitute_inline_summary_bindings(expression, &local_bindings),
                ))
            }
            Statement::Return(value) => {
                if summary.return_value.is_some() {
                    return None;
                }
                summary.return_value =
                    Some(substitute_inline_summary_bindings(value, &local_bindings));
            }
            Statement::Block { body } if body.is_empty() => {}
            _ => return None,
        }
    }

    Some(summary)
}

pub(in crate::backend::direct_wasm) fn rewrite_inline_function_summary_bindings(
    summary: &InlineFunctionSummary,
    bindings: &HashMap<String, Expression>,
) -> InlineFunctionSummary {
    InlineFunctionSummary {
        effects: summary
            .effects
            .iter()
            .map(|effect| match effect {
                InlineFunctionEffect::Assign { name, value } => InlineFunctionEffect::Assign {
                    name: name.clone(),
                    value: substitute_inline_summary_bindings(value, bindings),
                },
                InlineFunctionEffect::Update { name, op, prefix } => InlineFunctionEffect::Update {
                    name: name.clone(),
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

pub(in crate::backend::direct_wasm) fn static_expression_matches(
    lhs: &Expression,
    rhs: &Expression,
) -> bool {
    match (lhs, rhs) {
        (Expression::Number(left), Expression::Number(right)) => {
            (left.is_nan() && right.is_nan()) || left == right
        }
        _ => lhs == rhs,
    }
}

pub(in crate::backend::direct_wasm) fn expression_mentions_call_frame_state(
    expression: &Expression,
) -> bool {
    match expression {
        Expression::Identifier(name) => name == "arguments",
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
