use super::*;

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
