use super::*;

pub(in crate::backend::direct_wasm) fn collect_returned_member_value_bindings(
    statements: &[Statement],
) -> Vec<ReturnedMemberValueBinding> {
    if let Some(entries) = collect_returned_object_literal(statements) {
        return entries
            .into_iter()
            .filter_map(|entry| match entry {
                crate::ir::hir::ObjectEntry::Data {
                    key: Expression::String(property),
                    value,
                } => Some(ReturnedMemberValueBinding { property, value }),
                _ => None,
            })
            .collect();
    }

    let Some(returned_identifier) = collect_returned_identifier(statements) else {
        return Vec::new();
    };
    let local_aliases = collect_returned_member_local_aliases(statements);

    let mut bindings = HashMap::new();
    for statement in statements {
        collect_returned_member_value_bindings_from_statement(
            statement,
            &returned_identifier,
            &local_aliases,
            &mut bindings,
        );
    }

    bindings
        .into_iter()
        .map(|(property, value)| ReturnedMemberValueBinding { property, value })
        .collect()
}

pub(in crate::backend::direct_wasm) fn collect_returned_member_value_bindings_from_statement(
    statement: &Statement,
    returned_identifier: &str,
    local_aliases: &HashMap<String, Expression>,
    bindings: &mut HashMap<String, Expression>,
) {
    match statement {
        Statement::Block { body }
        | Statement::Labeled { body, .. }
        | Statement::With { body, .. } => {
            for statement in body {
                collect_returned_member_value_bindings_from_statement(
                    statement,
                    returned_identifier,
                    local_aliases,
                    bindings,
                );
            }
        }
        Statement::AssignMember {
            object,
            property,
            value,
        } => {
            if matches!(object, Expression::Identifier(name) if name == returned_identifier) {
                if let Expression::String(property_name) = property {
                    bindings.insert(property_name.clone(), value.clone());
                }
            }
            collect_returned_member_value_bindings_from_expression(
                object,
                returned_identifier,
                local_aliases,
                bindings,
            );
            collect_returned_member_value_bindings_from_expression(
                property,
                returned_identifier,
                local_aliases,
                bindings,
            );
            collect_returned_member_value_bindings_from_expression(
                value,
                returned_identifier,
                local_aliases,
                bindings,
            );
        }
        Statement::Var { value, .. }
        | Statement::Let { value, .. }
        | Statement::Assign { value, .. }
        | Statement::Expression(value)
        | Statement::Throw(value)
        | Statement::Return(value)
        | Statement::Yield { value }
        | Statement::YieldDelegate { value } => {
            collect_returned_member_value_bindings_from_expression(
                value,
                returned_identifier,
                local_aliases,
                bindings,
            );
        }
        Statement::Print { values } => {
            for value in values {
                collect_returned_member_value_bindings_from_expression(
                    value,
                    returned_identifier,
                    local_aliases,
                    bindings,
                );
            }
        }
        Statement::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_returned_member_value_bindings_from_expression(
                condition,
                returned_identifier,
                local_aliases,
                bindings,
            );
            for statement in then_branch {
                collect_returned_member_value_bindings_from_statement(
                    statement,
                    returned_identifier,
                    local_aliases,
                    bindings,
                );
            }
            for statement in else_branch {
                collect_returned_member_value_bindings_from_statement(
                    statement,
                    returned_identifier,
                    local_aliases,
                    bindings,
                );
            }
        }
        Statement::Try {
            body,
            catch_setup,
            catch_body,
            ..
        } => {
            for statement in body {
                collect_returned_member_value_bindings_from_statement(
                    statement,
                    returned_identifier,
                    local_aliases,
                    bindings,
                );
            }
            for statement in catch_setup {
                collect_returned_member_value_bindings_from_statement(
                    statement,
                    returned_identifier,
                    local_aliases,
                    bindings,
                );
            }
            for statement in catch_body {
                collect_returned_member_value_bindings_from_statement(
                    statement,
                    returned_identifier,
                    local_aliases,
                    bindings,
                );
            }
        }
        Statement::Switch {
            discriminant,
            cases,
            ..
        } => {
            collect_returned_member_value_bindings_from_expression(
                discriminant,
                returned_identifier,
                local_aliases,
                bindings,
            );
            for case in cases {
                if let Some(test) = &case.test {
                    collect_returned_member_value_bindings_from_expression(
                        test,
                        returned_identifier,
                        local_aliases,
                        bindings,
                    );
                }
                for statement in &case.body {
                    collect_returned_member_value_bindings_from_statement(
                        statement,
                        returned_identifier,
                        local_aliases,
                        bindings,
                    );
                }
            }
        }
        Statement::For {
            init,
            condition,
            update,
            break_hook,
            body,
            ..
        } => {
            for statement in init {
                collect_returned_member_value_bindings_from_statement(
                    statement,
                    returned_identifier,
                    local_aliases,
                    bindings,
                );
            }
            if let Some(condition) = condition {
                collect_returned_member_value_bindings_from_expression(
                    condition,
                    returned_identifier,
                    local_aliases,
                    bindings,
                );
            }
            if let Some(update) = update {
                collect_returned_member_value_bindings_from_expression(
                    update,
                    returned_identifier,
                    local_aliases,
                    bindings,
                );
            }
            if let Some(break_hook) = break_hook {
                collect_returned_member_value_bindings_from_expression(
                    break_hook,
                    returned_identifier,
                    local_aliases,
                    bindings,
                );
            }
            for statement in body {
                collect_returned_member_value_bindings_from_statement(
                    statement,
                    returned_identifier,
                    local_aliases,
                    bindings,
                );
            }
        }
        Statement::While {
            condition,
            break_hook,
            body,
            ..
        }
        | Statement::DoWhile {
            condition,
            break_hook,
            body,
            ..
        } => {
            collect_returned_member_value_bindings_from_expression(
                condition,
                returned_identifier,
                local_aliases,
                bindings,
            );
            if let Some(break_hook) = break_hook {
                collect_returned_member_value_bindings_from_expression(
                    break_hook,
                    returned_identifier,
                    local_aliases,
                    bindings,
                );
            }
            for statement in body {
                collect_returned_member_value_bindings_from_statement(
                    statement,
                    returned_identifier,
                    local_aliases,
                    bindings,
                );
            }
        }
        _ => {}
    }
}

pub(in crate::backend::direct_wasm) fn collect_returned_member_value_bindings_from_expression(
    expression: &Expression,
    returned_identifier: &str,
    local_aliases: &HashMap<String, Expression>,
    bindings: &mut HashMap<String, Expression>,
) {
    match expression {
        Expression::AssignMember {
            object,
            property,
            value,
        } => {
            if matches!(object.as_ref(), Expression::Identifier(name) if name == returned_identifier)
            {
                if let Expression::String(property_name) = property.as_ref() {
                    bindings.insert(property_name.clone(), (**value).clone());
                }
            }
            collect_returned_member_value_bindings_from_expression(
                object,
                returned_identifier,
                local_aliases,
                bindings,
            );
            collect_returned_member_value_bindings_from_expression(
                property,
                returned_identifier,
                local_aliases,
                bindings,
            );
            collect_returned_member_value_bindings_from_expression(
                value,
                returned_identifier,
                local_aliases,
                bindings,
            );
        }
        Expression::Call { callee, arguments }
        | Expression::SuperCall { callee, arguments }
        | Expression::New { callee, arguments } => {
            collect_returned_member_value_bindings_from_expression(
                callee,
                returned_identifier,
                local_aliases,
                bindings,
            );
            for argument in arguments {
                match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                        collect_returned_member_value_bindings_from_expression(
                            expression,
                            returned_identifier,
                            local_aliases,
                            bindings,
                        );
                    }
                }
            }

            let Expression::Member { object, property } = callee.as_ref() else {
                return;
            };
            if !matches!(object.as_ref(), Expression::Identifier(name) if name == "Object") {
                return;
            }
            if !matches!(property.as_ref(), Expression::String(name) if name == "defineProperty") {
                return;
            }
            let [
                CallArgument::Expression(target),
                CallArgument::Expression(property),
                CallArgument::Expression(descriptor),
                ..,
            ] = arguments.as_slice()
            else {
                return;
            };
            let Some(Expression::String(property_name)) =
                resolve_returned_member_value_property_key(
                    target,
                    property,
                    returned_identifier,
                    local_aliases,
                )
            else {
                return;
            };
            let Some(value) = resolve_returned_member_value_from_descriptor(descriptor) else {
                bindings.remove(&property_name);
                return;
            };
            bindings.insert(property_name, value);
        }
        Expression::Unary { expression, .. }
        | Expression::Await(expression)
        | Expression::EnumerateKeys(expression)
        | Expression::GetIterator(expression)
        | Expression::IteratorClose(expression) => {
            collect_returned_member_value_bindings_from_expression(
                expression,
                returned_identifier,
                local_aliases,
                bindings,
            );
        }
        Expression::Binary { left, right, .. } => {
            collect_returned_member_value_bindings_from_expression(
                left,
                returned_identifier,
                local_aliases,
                bindings,
            );
            collect_returned_member_value_bindings_from_expression(
                right,
                returned_identifier,
                local_aliases,
                bindings,
            );
        }
        Expression::Conditional {
            condition,
            then_expression,
            else_expression,
        } => {
            collect_returned_member_value_bindings_from_expression(
                condition,
                returned_identifier,
                local_aliases,
                bindings,
            );
            collect_returned_member_value_bindings_from_expression(
                then_expression,
                returned_identifier,
                local_aliases,
                bindings,
            );
            collect_returned_member_value_bindings_from_expression(
                else_expression,
                returned_identifier,
                local_aliases,
                bindings,
            );
        }
        Expression::Sequence(expressions) => {
            for expression in expressions {
                collect_returned_member_value_bindings_from_expression(
                    expression,
                    returned_identifier,
                    local_aliases,
                    bindings,
                );
            }
        }
        Expression::Member { object, property } => {
            collect_returned_member_value_bindings_from_expression(
                object,
                returned_identifier,
                local_aliases,
                bindings,
            );
            collect_returned_member_value_bindings_from_expression(
                property,
                returned_identifier,
                local_aliases,
                bindings,
            );
        }
        Expression::Assign { value, .. } | Expression::AssignSuperMember { value, .. } => {
            collect_returned_member_value_bindings_from_expression(
                value,
                returned_identifier,
                local_aliases,
                bindings,
            );
        }
        Expression::SuperMember { property } => {
            collect_returned_member_value_bindings_from_expression(
                property,
                returned_identifier,
                local_aliases,
                bindings,
            );
        }
        Expression::Array(elements) => {
            for element in elements {
                match element {
                    crate::ir::hir::ArrayElement::Expression(expression)
                    | crate::ir::hir::ArrayElement::Spread(expression) => {
                        collect_returned_member_value_bindings_from_expression(
                            expression,
                            returned_identifier,
                            local_aliases,
                            bindings,
                        );
                    }
                }
            }
        }
        Expression::Object(entries) => {
            for entry in entries {
                match entry {
                    crate::ir::hir::ObjectEntry::Data { key, value } => {
                        collect_returned_member_value_bindings_from_expression(
                            key,
                            returned_identifier,
                            local_aliases,
                            bindings,
                        );
                        collect_returned_member_value_bindings_from_expression(
                            value,
                            returned_identifier,
                            local_aliases,
                            bindings,
                        );
                    }
                    crate::ir::hir::ObjectEntry::Getter { key, getter }
                    | crate::ir::hir::ObjectEntry::Setter {
                        key,
                        setter: getter,
                    } => {
                        collect_returned_member_value_bindings_from_expression(
                            key,
                            returned_identifier,
                            local_aliases,
                            bindings,
                        );
                        collect_returned_member_value_bindings_from_expression(
                            getter,
                            returned_identifier,
                            local_aliases,
                            bindings,
                        );
                    }
                    crate::ir::hir::ObjectEntry::Spread(value) => {
                        collect_returned_member_value_bindings_from_expression(
                            value,
                            returned_identifier,
                            local_aliases,
                            bindings,
                        );
                    }
                }
            }
        }
        Expression::Number(_)
        | Expression::BigInt(_)
        | Expression::String(_)
        | Expression::Bool(_)
        | Expression::Null
        | Expression::Undefined
        | Expression::NewTarget
        | Expression::Identifier(_)
        | Expression::This
        | Expression::Sent
        | Expression::Update { .. } => {}
    }
}

pub(in crate::backend::direct_wasm) fn resolve_returned_member_value_property_key(
    object: &Expression,
    property: &Expression,
    returned_identifier: &str,
    local_aliases: &HashMap<String, Expression>,
) -> Option<Expression> {
    let resolved_property = resolve_returned_member_local_alias_expression(property, local_aliases);
    let property_key = match resolved_property {
        Expression::String(property_name) => Expression::String(property_name),
        _ => return None,
    };

    match object {
        Expression::Identifier(name) if name == returned_identifier => Some(property_key),
        _ => None,
    }
}

pub(in crate::backend::direct_wasm) fn resolve_returned_member_value_from_descriptor(
    descriptor: &Expression,
) -> Option<Expression> {
    resolve_property_descriptor_definition(descriptor)?.value
}
