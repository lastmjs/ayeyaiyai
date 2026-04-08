use super::*;

pub(in crate::backend::direct_wasm) fn collect_eval_var_names(
    program: &Program,
) -> HashSet<String> {
    let mut names = HashSet::new();
    collect_eval_var_names_from_statements(&program.statements, &mut names);
    names.extend(
        program
            .functions
            .iter()
            .filter(|function| function.register_global)
            .map(|function| function.name.clone()),
    );
    names
}

pub(in crate::backend::direct_wasm) fn collect_eval_var_names_from_statements(
    statements: &[Statement],
    names: &mut HashSet<String>,
) {
    for statement in statements {
        match statement {
            Statement::Var { name, .. } => {
                names.insert(name.clone());
            }
            Statement::Declaration { body }
            | Statement::Block { body }
            | Statement::Labeled { body, .. }
            | Statement::With { body, .. }
            | Statement::While { body, .. }
            | Statement::DoWhile { body, .. } => {
                collect_eval_var_names_from_statements(body, names);
            }
            Statement::If {
                then_branch,
                else_branch,
                ..
            } => {
                collect_eval_var_names_from_statements(then_branch, names);
                collect_eval_var_names_from_statements(else_branch, names);
            }
            Statement::Try {
                body,
                catch_setup,
                catch_body,
                ..
            } => {
                collect_eval_var_names_from_statements(body, names);
                collect_eval_var_names_from_statements(catch_setup, names);
                collect_eval_var_names_from_statements(catch_body, names);
            }
            Statement::Switch { cases, .. } => {
                for case in cases {
                    collect_eval_var_names_from_statements(&case.body, names);
                }
            }
            Statement::For { init, body, .. } => {
                collect_eval_var_names_from_statements(init, names);
                collect_eval_var_names_from_statements(body, names);
            }
            Statement::Let { .. }
            | Statement::Assign { .. }
            | Statement::AssignMember { .. }
            | Statement::Print { .. }
            | Statement::Expression(_)
            | Statement::Throw(_)
            | Statement::Return(_)
            | Statement::Break { .. }
            | Statement::Continue { .. }
            | Statement::Yield { .. }
            | Statement::YieldDelegate { .. } => {}
        }
    }
}

pub(in crate::backend::direct_wasm) fn eval_statements_declare_var_arguments(
    statements: &[Statement],
) -> bool {
    statements.iter().any(eval_statement_declares_var_arguments)
}

pub(in crate::backend::direct_wasm) fn eval_statement_declares_var_arguments(
    statement: &Statement,
) -> bool {
    match statement {
        Statement::Var { name, .. } => name == "arguments",
        Statement::Declaration { body }
        | Statement::Block { body }
        | Statement::Labeled { body, .. }
        | Statement::With { body, .. }
        | Statement::While { body, .. }
        | Statement::DoWhile { body, .. } => eval_statements_declare_var_arguments(body),
        Statement::If {
            then_branch,
            else_branch,
            ..
        } => {
            eval_statements_declare_var_arguments(then_branch)
                || eval_statements_declare_var_arguments(else_branch)
        }
        Statement::Try {
            body,
            catch_setup,
            catch_body,
            ..
        } => {
            eval_statements_declare_var_arguments(body)
                || eval_statements_declare_var_arguments(catch_setup)
                || eval_statements_declare_var_arguments(catch_body)
        }
        Statement::Switch { cases, .. } => cases
            .iter()
            .any(|case| eval_statements_declare_var_arguments(&case.body)),
        Statement::For { init, body, .. } => {
            eval_statements_declare_var_arguments(init)
                || eval_statements_declare_var_arguments(body)
        }
        Statement::Let { .. }
        | Statement::Assign { .. }
        | Statement::AssignMember { .. }
        | Statement::Print { .. }
        | Statement::Expression(_)
        | Statement::Throw(_)
        | Statement::Return(_)
        | Statement::Break { .. }
        | Statement::Continue { .. }
        | Statement::Yield { .. }
        | Statement::YieldDelegate { .. } => false,
    }
}
