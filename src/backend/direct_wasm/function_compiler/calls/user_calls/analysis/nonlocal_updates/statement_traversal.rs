use super::*;

use super::expression_traversal::collect_updated_names_from_expression;

pub(super) fn collect_updated_names_from_statement(
    statement: &Statement,
    names: &mut HashSet<String>,
) {
    match statement {
        Statement::Declaration { body }
        | Statement::Block { body }
        | Statement::Labeled { body, .. }
        | Statement::With { body, .. } => {
            for statement in body {
                collect_updated_names_from_statement(statement, names);
            }
        }
        Statement::Expression(expression)
        | Statement::Return(expression)
        | Statement::Throw(expression)
        | Statement::Yield { value: expression }
        | Statement::YieldDelegate { value: expression } => {
            collect_updated_names_from_expression(expression, names);
        }
        Statement::Var { value, .. } | Statement::Let { value, .. } => {
            collect_updated_names_from_expression(value, names);
        }
        Statement::Print { values } => {
            for value in values {
                collect_updated_names_from_expression(value, names);
            }
        }
        Statement::Assign { value, .. } => {
            collect_updated_names_from_expression(value, names);
        }
        Statement::AssignMember {
            object,
            property,
            value,
        } => {
            collect_updated_names_from_expression(object, names);
            collect_updated_names_from_expression(property, names);
            collect_updated_names_from_expression(value, names);
        }
        Statement::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_updated_names_from_expression(condition, names);
            for statement in then_branch {
                collect_updated_names_from_statement(statement, names);
            }
            for statement in else_branch {
                collect_updated_names_from_statement(statement, names);
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
            collect_updated_names_from_expression(condition, names);
            if let Some(break_hook) = break_hook {
                collect_updated_names_from_expression(break_hook, names);
            }
            for statement in body {
                collect_updated_names_from_statement(statement, names);
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
                collect_updated_names_from_statement(statement, names);
            }
            if let Some(condition) = condition {
                collect_updated_names_from_expression(condition, names);
            }
            if let Some(update) = update {
                collect_updated_names_from_expression(update, names);
            }
            if let Some(break_hook) = break_hook {
                collect_updated_names_from_expression(break_hook, names);
            }
            for statement in body {
                collect_updated_names_from_statement(statement, names);
            }
        }
        Statement::Try {
            body,
            catch_setup,
            catch_body,
            ..
        } => {
            for statement in body {
                collect_updated_names_from_statement(statement, names);
            }
            for statement in catch_setup {
                collect_updated_names_from_statement(statement, names);
            }
            for statement in catch_body {
                collect_updated_names_from_statement(statement, names);
            }
        }
        Statement::Switch {
            discriminant,
            cases,
            ..
        } => {
            collect_updated_names_from_expression(discriminant, names);
            for case in cases {
                if let Some(test) = &case.test {
                    collect_updated_names_from_expression(test, names);
                }
                for statement in &case.body {
                    collect_updated_names_from_statement(statement, names);
                }
            }
        }
        Statement::Break { .. } | Statement::Continue { .. } => {}
    }
}
