use super::super::*;

pub(in crate::backend::direct_wasm) fn collect_returned_member_local_aliases_from_statement(
    statement: &Statement,
    aliases: &mut HashMap<String, Expression>,
) {
    match statement {
        Statement::Declaration { body }
        | Statement::Block { body }
        | Statement::Labeled { body, .. }
        | Statement::With { body, .. } => {
            for statement in body {
                collect_returned_member_local_aliases_from_statement(statement, aliases);
            }
        }
        Statement::Var { name, value } | Statement::Let { name, value, .. } => {
            aliases.insert(
                name.clone(),
                super::resolve_returned_member_local_alias_expression(value, aliases),
            );
        }
        Statement::Assign { name, value } => {
            aliases.insert(
                name.clone(),
                super::resolve_returned_member_local_alias_expression(value, aliases),
            );
        }
        Statement::If {
            condition,
            then_branch,
            else_branch,
        } => {
            super::collect_returned_member_local_aliases_from_expression(condition, aliases);
            for statement in then_branch {
                collect_returned_member_local_aliases_from_statement(statement, aliases);
            }
            for statement in else_branch {
                collect_returned_member_local_aliases_from_statement(statement, aliases);
            }
        }
        Statement::Try {
            body,
            catch_setup,
            catch_body,
            ..
        } => {
            for statement in body {
                collect_returned_member_local_aliases_from_statement(statement, aliases);
            }
            for statement in catch_setup {
                collect_returned_member_local_aliases_from_statement(statement, aliases);
            }
            for statement in catch_body {
                collect_returned_member_local_aliases_from_statement(statement, aliases);
            }
        }
        Statement::Switch {
            discriminant,
            cases,
            ..
        } => {
            super::collect_returned_member_local_aliases_from_expression(discriminant, aliases);
            for case in cases {
                if let Some(test) = &case.test {
                    super::collect_returned_member_local_aliases_from_expression(test, aliases);
                }
                for statement in &case.body {
                    collect_returned_member_local_aliases_from_statement(statement, aliases);
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
                collect_returned_member_local_aliases_from_statement(statement, aliases);
            }
            if let Some(condition) = condition {
                super::collect_returned_member_local_aliases_from_expression(condition, aliases);
            }
            if let Some(update) = update {
                super::collect_returned_member_local_aliases_from_expression(update, aliases);
            }
            if let Some(break_hook) = break_hook {
                super::collect_returned_member_local_aliases_from_expression(break_hook, aliases);
            }
            for statement in body {
                collect_returned_member_local_aliases_from_statement(statement, aliases);
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
            super::collect_returned_member_local_aliases_from_expression(condition, aliases);
            if let Some(break_hook) = break_hook {
                super::collect_returned_member_local_aliases_from_expression(break_hook, aliases);
            }
            for statement in body {
                collect_returned_member_local_aliases_from_statement(statement, aliases);
            }
        }
        Statement::Expression(expression)
        | Statement::Throw(expression)
        | Statement::Return(expression)
        | Statement::Yield { value: expression }
        | Statement::YieldDelegate { value: expression } => {
            super::collect_returned_member_local_aliases_from_expression(expression, aliases);
        }
        Statement::Print { values } => {
            for value in values {
                super::collect_returned_member_local_aliases_from_expression(value, aliases);
            }
        }
        Statement::AssignMember {
            object,
            property,
            value,
        } => {
            super::collect_returned_member_local_aliases_from_expression(object, aliases);
            super::collect_returned_member_local_aliases_from_expression(property, aliases);
            super::collect_returned_member_local_aliases_from_expression(value, aliases);
        }
        Statement::Break { .. } | Statement::Continue { .. } => {}
    }
}
