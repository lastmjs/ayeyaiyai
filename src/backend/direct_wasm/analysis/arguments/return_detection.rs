use super::*;

pub(in crate::backend::direct_wasm) fn function_returns_arguments_object(
    statements: &[Statement],
) -> bool {
    statements.iter().any(statement_returns_arguments_object)
}

pub(in crate::backend::direct_wasm) fn statement_returns_arguments_object(
    statement: &Statement,
) -> bool {
    match statement {
        Statement::Return(Expression::Identifier(name)) => name == "arguments",
        Statement::Declaration { body }
        | Statement::Block { body }
        | Statement::Labeled { body, .. } => body.iter().any(statement_returns_arguments_object),
        Statement::If {
            then_branch,
            else_branch,
            ..
        } => {
            then_branch.iter().any(statement_returns_arguments_object)
                || else_branch.iter().any(statement_returns_arguments_object)
        }
        Statement::Try {
            body,
            catch_setup,
            catch_body,
            ..
        } => {
            body.iter().any(statement_returns_arguments_object)
                || catch_setup.iter().any(statement_returns_arguments_object)
                || catch_body.iter().any(statement_returns_arguments_object)
        }
        Statement::Switch { cases, .. } => cases
            .iter()
            .any(|case| case.body.iter().any(statement_returns_arguments_object)),
        Statement::For { init, body, .. } => {
            init.iter().any(statement_returns_arguments_object)
                || body.iter().any(statement_returns_arguments_object)
        }
        Statement::While { body, .. } | Statement::DoWhile { body, .. } => {
            body.iter().any(statement_returns_arguments_object)
        }
        _ => false,
    }
}
