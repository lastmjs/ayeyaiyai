use super::*;

pub(in crate::backend::direct_wasm) fn collect_implicit_globals_from_statements(
    statements: &[Statement],
    strict: bool,
    scope: &HashSet<String>,
    names: &mut BTreeSet<String>,
) -> DirectResult<()> {
    for statement in statements {
        collect_implicit_globals_from_statement(statement, strict, scope, names)?;
    }
    Ok(())
}

pub(in crate::backend::direct_wasm) fn collect_implicit_globals_from_statement(
    statement: &Statement,
    strict: bool,
    scope: &HashSet<String>,
    names: &mut BTreeSet<String>,
) -> DirectResult<()> {
    match statement {
        Statement::Declaration { body }
        | Statement::Block { body }
        | Statement::Labeled { body, .. } => {
            collect_implicit_globals_from_statements(body, strict, scope, names)
        }
        Statement::Var { value, .. }
        | Statement::Let { value, .. }
        | Statement::Expression(value)
        | Statement::Throw(value)
        | Statement::Return(value)
        | Statement::Yield { value }
        | Statement::YieldDelegate { value } => {
            collect_implicit_globals_from_expression(value, strict, scope, names)
        }
        Statement::Assign { name, value } => {
            if !strict && !scope.contains(name) {
                names.insert(name.clone());
            }
            collect_implicit_globals_from_expression(value, strict, scope, names)
        }
        Statement::AssignMember {
            object,
            property,
            value,
        } => {
            collect_implicit_globals_from_expression(object, strict, scope, names)?;
            collect_implicit_globals_from_expression(property, strict, scope, names)?;
            collect_implicit_globals_from_expression(value, strict, scope, names)
        }
        Statement::Print { values } => {
            for value in values {
                collect_implicit_globals_from_expression(value, strict, scope, names)?;
            }
            Ok(())
        }
        Statement::With { object, body } => {
            collect_implicit_globals_from_expression(object, strict, scope, names)?;
            collect_implicit_globals_from_statements(body, strict, scope, names)
        }
        Statement::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_implicit_globals_from_expression(condition, strict, scope, names)?;
            collect_implicit_globals_from_statements(then_branch, strict, scope, names)?;
            collect_implicit_globals_from_statements(else_branch, strict, scope, names)
        }
        Statement::Try {
            body,
            catch_setup,
            catch_body,
            ..
        } => {
            collect_implicit_globals_from_statements(body, strict, scope, names)?;
            collect_implicit_globals_from_statements(catch_setup, strict, scope, names)?;
            collect_implicit_globals_from_statements(catch_body, strict, scope, names)
        }
        Statement::Switch {
            discriminant,
            cases,
            ..
        } => {
            collect_implicit_globals_from_expression(discriminant, strict, scope, names)?;
            for case in cases {
                if let Some(test) = &case.test {
                    collect_implicit_globals_from_expression(test, strict, scope, names)?;
                }
                collect_implicit_globals_from_statements(&case.body, strict, scope, names)?;
            }
            Ok(())
        }
        Statement::For {
            init,
            condition,
            update,
            break_hook,
            body,
            ..
        } => {
            collect_implicit_globals_from_statements(init, strict, scope, names)?;
            if let Some(condition) = condition {
                collect_implicit_globals_from_expression(condition, strict, scope, names)?;
            }
            if let Some(update) = update {
                collect_implicit_globals_from_expression(update, strict, scope, names)?;
            }
            if let Some(break_hook) = break_hook {
                collect_implicit_globals_from_expression(break_hook, strict, scope, names)?;
            }
            collect_implicit_globals_from_statements(body, strict, scope, names)
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
            collect_implicit_globals_from_expression(condition, strict, scope, names)?;
            if let Some(break_hook) = break_hook {
                collect_implicit_globals_from_expression(break_hook, strict, scope, names)?;
            }
            collect_implicit_globals_from_statements(body, strict, scope, names)
        }
        Statement::Break { .. } | Statement::Continue { .. } => Ok(()),
    }
}
