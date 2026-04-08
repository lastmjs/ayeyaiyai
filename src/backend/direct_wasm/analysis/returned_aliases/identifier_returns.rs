use super::*;

pub(in crate::backend::direct_wasm) fn collect_returned_identifier(
    statements: &[Statement],
) -> Option<String> {
    statements
        .iter()
        .rev()
        .find_map(collect_returned_identifier_from_statement)
}

pub(in crate::backend::direct_wasm) fn collect_returned_identifier_source_expression(
    statements: &[Statement],
) -> Option<Expression> {
    let returned_identifier = collect_returned_identifier(statements)?;
    statements.iter().rev().find_map(|statement| {
        collect_returned_identifier_source_expression_from_statement(
            statement,
            &returned_identifier,
        )
    })
}

pub(in crate::backend::direct_wasm) fn collect_returned_identifier_with_scope_objects(
    statements: &[Statement],
    returned_identifier: &str,
) -> Option<Vec<Expression>> {
    statements.iter().rev().find_map(|statement| {
        collect_returned_identifier_with_scope_objects_from_statement(
            statement,
            returned_identifier,
            &[],
        )
    })
}

pub(in crate::backend::direct_wasm) fn collect_returned_identifier_from_statement(
    statement: &Statement,
) -> Option<String> {
    match statement {
        Statement::Return(Expression::Identifier(name)) => Some(name.clone()),
        Statement::Block { body }
        | Statement::Labeled { body, .. }
        | Statement::With { body, .. } => collect_returned_identifier(body),
        Statement::If {
            then_branch,
            else_branch,
            ..
        } => collect_returned_identifier(then_branch)
            .or_else(|| collect_returned_identifier(else_branch)),
        Statement::Try {
            body,
            catch_setup,
            catch_body,
            ..
        } => collect_returned_identifier(body)
            .or_else(|| collect_returned_identifier(catch_setup))
            .or_else(|| collect_returned_identifier(catch_body)),
        Statement::Switch { cases, .. } => cases
            .iter()
            .rev()
            .find_map(|case| collect_returned_identifier(&case.body)),
        Statement::For { init, body, .. } => {
            collect_returned_identifier(body).or_else(|| collect_returned_identifier(init))
        }
        Statement::While { body, .. } | Statement::DoWhile { body, .. } => {
            collect_returned_identifier(body)
        }
        _ => None,
    }
}

pub(in crate::backend::direct_wasm) fn collect_returned_identifier_source_expression_from_statement(
    statement: &Statement,
    returned_identifier: &str,
) -> Option<Expression> {
    match statement {
        Statement::Var { name, value }
        | Statement::Let { name, value, .. }
        | Statement::Assign { name, value }
            if name == returned_identifier =>
        {
            Some(value.clone())
        }
        Statement::Block { body }
        | Statement::Labeled { body, .. }
        | Statement::With { body, .. } => collect_returned_identifier_source_expression(body),
        Statement::If {
            then_branch,
            else_branch,
            ..
        } => collect_returned_identifier_source_expression(then_branch)
            .or_else(|| collect_returned_identifier_source_expression(else_branch)),
        Statement::Try {
            body,
            catch_setup,
            catch_body,
            ..
        } => collect_returned_identifier_source_expression(body)
            .or_else(|| collect_returned_identifier_source_expression(catch_setup))
            .or_else(|| collect_returned_identifier_source_expression(catch_body)),
        Statement::Switch { cases, .. } => cases
            .iter()
            .rev()
            .find_map(|case| collect_returned_identifier_source_expression(&case.body)),
        Statement::For { init, body, .. } => collect_returned_identifier_source_expression(body)
            .or_else(|| collect_returned_identifier_source_expression(init)),
        Statement::While { body, .. } | Statement::DoWhile { body, .. } => {
            collect_returned_identifier_source_expression(body)
        }
        _ => None,
    }
}

pub(in crate::backend::direct_wasm) fn collect_returned_identifier_with_scope_objects_from_statement(
    statement: &Statement,
    returned_identifier: &str,
    active_with_scopes: &[Expression],
) -> Option<Vec<Expression>> {
    match statement {
        Statement::Return(Expression::Identifier(name)) if name == returned_identifier => {
            Some(active_with_scopes.to_vec())
        }
        Statement::Block { body } | Statement::Labeled { body, .. } => {
            body.iter().rev().find_map(|statement| {
                collect_returned_identifier_with_scope_objects_from_statement(
                    statement,
                    returned_identifier,
                    active_with_scopes,
                )
            })
        }
        Statement::With { object, body } => {
            let mut nested_scopes = active_with_scopes.to_vec();
            nested_scopes.push(object.clone());
            body.iter().rev().find_map(|statement| {
                collect_returned_identifier_with_scope_objects_from_statement(
                    statement,
                    returned_identifier,
                    &nested_scopes,
                )
            })
        }
        Statement::If {
            then_branch,
            else_branch,
            ..
        } => then_branch
            .iter()
            .rev()
            .find_map(|statement| {
                collect_returned_identifier_with_scope_objects_from_statement(
                    statement,
                    returned_identifier,
                    active_with_scopes,
                )
            })
            .or_else(|| {
                else_branch.iter().rev().find_map(|statement| {
                    collect_returned_identifier_with_scope_objects_from_statement(
                        statement,
                        returned_identifier,
                        active_with_scopes,
                    )
                })
            }),
        Statement::Try {
            body,
            catch_setup,
            catch_body,
            ..
        } => body
            .iter()
            .rev()
            .find_map(|statement| {
                collect_returned_identifier_with_scope_objects_from_statement(
                    statement,
                    returned_identifier,
                    active_with_scopes,
                )
            })
            .or_else(|| {
                catch_setup.iter().rev().find_map(|statement| {
                    collect_returned_identifier_with_scope_objects_from_statement(
                        statement,
                        returned_identifier,
                        active_with_scopes,
                    )
                })
            })
            .or_else(|| {
                catch_body.iter().rev().find_map(|statement| {
                    collect_returned_identifier_with_scope_objects_from_statement(
                        statement,
                        returned_identifier,
                        active_with_scopes,
                    )
                })
            }),
        Statement::Switch { cases, .. } => cases.iter().rev().find_map(|case| {
            case.body.iter().rev().find_map(|statement| {
                collect_returned_identifier_with_scope_objects_from_statement(
                    statement,
                    returned_identifier,
                    active_with_scopes,
                )
            })
        }),
        Statement::For { init, body, .. } => body
            .iter()
            .rev()
            .find_map(|statement| {
                collect_returned_identifier_with_scope_objects_from_statement(
                    statement,
                    returned_identifier,
                    active_with_scopes,
                )
            })
            .or_else(|| {
                init.iter().rev().find_map(|statement| {
                    collect_returned_identifier_with_scope_objects_from_statement(
                        statement,
                        returned_identifier,
                        active_with_scopes,
                    )
                })
            }),
        Statement::While { body, .. } | Statement::DoWhile { body, .. } => {
            body.iter().rev().find_map(|statement| {
                collect_returned_identifier_with_scope_objects_from_statement(
                    statement,
                    returned_identifier,
                    active_with_scopes,
                )
            })
        }
        _ => None,
    }
}
