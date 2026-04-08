use super::*;

pub(in crate::backend::direct_wasm) fn collect_returned_member_function_bindings_from_statement(
    statement: &Statement,
    returned_identifier: &str,
    function_names: &HashSet<String>,
    bindings: &mut HashMap<ReturnedMemberFunctionBindingKey, LocalFunctionBinding>,
) {
    match statement {
        Statement::Block { body }
        | Statement::Labeled { body, .. }
        | Statement::With { body, .. } => {
            for statement in body {
                collect_returned_member_function_bindings_from_statement(
                    statement,
                    returned_identifier,
                    function_names,
                    bindings,
                );
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
            collect_returned_member_function_bindings_from_expression(
                value,
                returned_identifier,
                function_names,
                bindings,
            );
        }
        Statement::AssignMember {
            object,
            property,
            value,
        } => {
            collect_returned_member_function_bindings_from_expression(
                object,
                returned_identifier,
                function_names,
                bindings,
            );
            collect_returned_member_function_bindings_from_expression(
                property,
                returned_identifier,
                function_names,
                bindings,
            );
            collect_returned_member_function_bindings_from_expression(
                value,
                returned_identifier,
                function_names,
                bindings,
            );
        }
        Statement::Print { values } => {
            for value in values {
                collect_returned_member_function_bindings_from_expression(
                    value,
                    returned_identifier,
                    function_names,
                    bindings,
                );
            }
        }
        Statement::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_returned_member_function_bindings_from_expression(
                condition,
                returned_identifier,
                function_names,
                bindings,
            );
            for statement in then_branch {
                collect_returned_member_function_bindings_from_statement(
                    statement,
                    returned_identifier,
                    function_names,
                    bindings,
                );
            }
            for statement in else_branch {
                collect_returned_member_function_bindings_from_statement(
                    statement,
                    returned_identifier,
                    function_names,
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
                collect_returned_member_function_bindings_from_statement(
                    statement,
                    returned_identifier,
                    function_names,
                    bindings,
                );
            }
            for statement in catch_setup {
                collect_returned_member_function_bindings_from_statement(
                    statement,
                    returned_identifier,
                    function_names,
                    bindings,
                );
            }
            for statement in catch_body {
                collect_returned_member_function_bindings_from_statement(
                    statement,
                    returned_identifier,
                    function_names,
                    bindings,
                );
            }
        }
        Statement::Switch {
            discriminant,
            cases,
            ..
        } => {
            collect_returned_member_function_bindings_from_expression(
                discriminant,
                returned_identifier,
                function_names,
                bindings,
            );
            for case in cases {
                if let Some(test) = &case.test {
                    collect_returned_member_function_bindings_from_expression(
                        test,
                        returned_identifier,
                        function_names,
                        bindings,
                    );
                }
                for statement in &case.body {
                    collect_returned_member_function_bindings_from_statement(
                        statement,
                        returned_identifier,
                        function_names,
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
                collect_returned_member_function_bindings_from_statement(
                    statement,
                    returned_identifier,
                    function_names,
                    bindings,
                );
            }
            if let Some(condition) = condition {
                collect_returned_member_function_bindings_from_expression(
                    condition,
                    returned_identifier,
                    function_names,
                    bindings,
                );
            }
            if let Some(update) = update {
                collect_returned_member_function_bindings_from_expression(
                    update,
                    returned_identifier,
                    function_names,
                    bindings,
                );
            }
            if let Some(break_hook) = break_hook {
                collect_returned_member_function_bindings_from_expression(
                    break_hook,
                    returned_identifier,
                    function_names,
                    bindings,
                );
            }
            for statement in body {
                collect_returned_member_function_bindings_from_statement(
                    statement,
                    returned_identifier,
                    function_names,
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
            collect_returned_member_function_bindings_from_expression(
                condition,
                returned_identifier,
                function_names,
                bindings,
            );
            if let Some(break_hook) = break_hook {
                collect_returned_member_function_bindings_from_expression(
                    break_hook,
                    returned_identifier,
                    function_names,
                    bindings,
                );
            }
            for statement in body {
                collect_returned_member_function_bindings_from_statement(
                    statement,
                    returned_identifier,
                    function_names,
                    bindings,
                );
            }
        }
        _ => {}
    }
}
