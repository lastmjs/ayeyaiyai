use super::*;

pub(in crate::backend::direct_wasm) fn collect_returned_object_literal(
    statements: &[Statement],
) -> Option<Vec<ObjectEntry>> {
    statements
        .iter()
        .rev()
        .find_map(collect_returned_object_literal_from_statement)
}

pub(in crate::backend::direct_wasm) fn collect_returned_object_literal_from_statement(
    statement: &Statement,
) -> Option<Vec<ObjectEntry>> {
    match statement {
        Statement::Return(Expression::Object(entries)) => Some(entries.clone()),
        Statement::Block { body }
        | Statement::Labeled { body, .. }
        | Statement::With { body, .. } => collect_returned_object_literal(body),
        Statement::If {
            then_branch,
            else_branch,
            ..
        } => collect_returned_object_literal(then_branch)
            .or_else(|| collect_returned_object_literal(else_branch)),
        Statement::Try {
            body,
            catch_setup,
            catch_body,
            ..
        } => collect_returned_object_literal(body)
            .or_else(|| collect_returned_object_literal(catch_setup))
            .or_else(|| collect_returned_object_literal(catch_body)),
        Statement::Switch { cases, .. } => cases
            .iter()
            .rev()
            .find_map(|case| collect_returned_object_literal(&case.body)),
        Statement::For { init, body, .. } => {
            collect_returned_object_literal(body).or_else(|| collect_returned_object_literal(init))
        }
        Statement::While { body, .. } | Statement::DoWhile { body, .. } => {
            collect_returned_object_literal(body)
        }
        _ => None,
    }
}
