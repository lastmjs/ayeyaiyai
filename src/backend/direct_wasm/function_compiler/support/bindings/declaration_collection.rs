use super::*;

pub(in crate::backend::direct_wasm) fn collect_declared_bindings_from_statements_recursive(
    statements: &[Statement],
) -> HashSet<String> {
    let mut bindings = HashSet::new();
    for statement in statements {
        collect_declared_bindings_from_statement(statement, &mut bindings);
    }
    bindings
}

pub(in crate::backend::direct_wasm) fn collect_declared_bindings_from_statement(
    statement: &Statement,
    bindings: &mut HashSet<String>,
) {
    match statement {
        Statement::Declaration { body }
        | Statement::Block { body }
        | Statement::Labeled { body, .. } => {
            for statement in body {
                collect_declared_bindings_from_statement(statement, bindings);
            }
        }
        Statement::Var { name, .. } | Statement::Let { name, .. } => {
            bindings.insert(name.clone());
        }
        Statement::If {
            then_branch,
            else_branch,
            ..
        } => {
            for statement in then_branch {
                collect_declared_bindings_from_statement(statement, bindings);
            }
            for statement in else_branch {
                collect_declared_bindings_from_statement(statement, bindings);
            }
        }
        Statement::With { body, .. }
        | Statement::While { body, .. }
        | Statement::DoWhile { body, .. } => {
            for statement in body {
                collect_declared_bindings_from_statement(statement, bindings);
            }
        }
        Statement::Try {
            body,
            catch_binding,
            catch_setup,
            catch_body,
        } => {
            for statement in body {
                collect_declared_bindings_from_statement(statement, bindings);
            }
            if let Some(catch_binding) = catch_binding {
                bindings.insert(catch_binding.clone());
            }
            for statement in catch_setup {
                collect_declared_bindings_from_statement(statement, bindings);
            }
            for statement in catch_body {
                collect_declared_bindings_from_statement(statement, bindings);
            }
        }
        Statement::Switch {
            bindings: names,
            cases,
            ..
        } => {
            bindings.extend(names.iter().cloned());
            for case in cases {
                for statement in &case.body {
                    collect_declared_bindings_from_statement(statement, bindings);
                }
            }
        }
        Statement::For {
            init,
            per_iteration_bindings,
            body,
            ..
        } => {
            bindings.extend(per_iteration_bindings.iter().cloned());
            for statement in init {
                collect_declared_bindings_from_statement(statement, bindings);
            }
            for statement in body {
                collect_declared_bindings_from_statement(statement, bindings);
            }
        }
        Statement::Assign { .. }
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
