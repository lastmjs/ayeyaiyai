use super::*;

pub(in crate::backend::direct_wasm) fn collect_arguments_usage_from_statement(
    statement: &Statement,
    indexed_slots: &mut HashSet<u32>,
    track_all_slots: &mut bool,
) {
    match statement {
        Statement::Declaration { body }
        | Statement::Block { body }
        | Statement::Labeled { body, .. } => {
            for statement in body {
                collect_arguments_usage_from_statement(statement, indexed_slots, track_all_slots);
            }
        }
        Statement::Var { value, .. }
        | Statement::Let { value, .. }
        | Statement::Assign { value, .. }
        | Statement::Expression(value)
        | Statement::Throw(value)
        | Statement::Return(value)
        | Statement::Yield { value }
        | Statement::YieldDelegate { value } => {
            collect_arguments_usage_from_expression(value, indexed_slots, track_all_slots);
        }
        Statement::AssignMember {
            object,
            property,
            value,
        } => {
            collect_arguments_usage_from_expression(object, indexed_slots, track_all_slots);
            collect_arguments_usage_from_expression(property, indexed_slots, track_all_slots);
            collect_arguments_usage_from_expression(value, indexed_slots, track_all_slots);
        }
        Statement::Print { values } => {
            for value in values {
                collect_arguments_usage_from_expression(value, indexed_slots, track_all_slots);
            }
        }
        Statement::With { object, body } => {
            collect_arguments_usage_from_expression(object, indexed_slots, track_all_slots);
            for statement in body {
                collect_arguments_usage_from_statement(statement, indexed_slots, track_all_slots);
            }
        }
        Statement::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_arguments_usage_from_expression(condition, indexed_slots, track_all_slots);
            for statement in then_branch {
                collect_arguments_usage_from_statement(statement, indexed_slots, track_all_slots);
            }
            for statement in else_branch {
                collect_arguments_usage_from_statement(statement, indexed_slots, track_all_slots);
            }
        }
        Statement::Try {
            body,
            catch_setup,
            catch_body,
            ..
        } => {
            for statement in body {
                collect_arguments_usage_from_statement(statement, indexed_slots, track_all_slots);
            }
            for statement in catch_setup {
                collect_arguments_usage_from_statement(statement, indexed_slots, track_all_slots);
            }
            for statement in catch_body {
                collect_arguments_usage_from_statement(statement, indexed_slots, track_all_slots);
            }
        }
        Statement::Switch {
            discriminant,
            cases,
            ..
        } => {
            collect_arguments_usage_from_expression(discriminant, indexed_slots, track_all_slots);
            for case in cases {
                if let Some(test) = &case.test {
                    collect_arguments_usage_from_expression(test, indexed_slots, track_all_slots);
                }
                for statement in &case.body {
                    collect_arguments_usage_from_statement(
                        statement,
                        indexed_slots,
                        track_all_slots,
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
                collect_arguments_usage_from_statement(statement, indexed_slots, track_all_slots);
            }
            if let Some(condition) = condition {
                collect_arguments_usage_from_expression(condition, indexed_slots, track_all_slots);
            }
            if let Some(update) = update {
                collect_arguments_usage_from_expression(update, indexed_slots, track_all_slots);
            }
            if let Some(break_hook) = break_hook {
                collect_arguments_usage_from_expression(break_hook, indexed_slots, track_all_slots);
            }
            for statement in body {
                collect_arguments_usage_from_statement(statement, indexed_slots, track_all_slots);
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
            collect_arguments_usage_from_expression(condition, indexed_slots, track_all_slots);
            if let Some(break_hook) = break_hook {
                collect_arguments_usage_from_expression(break_hook, indexed_slots, track_all_slots);
            }
            for statement in body {
                collect_arguments_usage_from_statement(statement, indexed_slots, track_all_slots);
            }
        }
        Statement::Break { .. } | Statement::Continue { .. } => {}
    }
}
