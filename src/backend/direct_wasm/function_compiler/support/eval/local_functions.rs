use super::*;

pub(in crate::backend::direct_wasm) fn collect_eval_local_function_declarations(
    statements: &[Statement],
    local_function_names: &HashSet<String>,
) -> HashMap<String, String> {
    let mut declarations = HashMap::new();
    collect_eval_local_function_declarations_from_statements(
        statements,
        local_function_names,
        &mut declarations,
    );
    declarations
}

pub(in crate::backend::direct_wasm) fn is_eval_local_function_candidate(
    function: &FunctionDeclaration,
) -> bool {
    !function.register_global && function.name.starts_with("__ayy_fnstmt_")
}

pub(in crate::backend::direct_wasm) fn collect_eval_local_function_declarations_from_statements(
    statements: &[Statement],
    local_function_names: &HashSet<String>,
    declarations: &mut HashMap<String, String>,
) {
    for statement in statements {
        if let Some((binding_name, function_name)) =
            eval_local_function_declaration_from_statement(statement, local_function_names)
        {
            declarations.insert(binding_name, function_name);
        }
        match statement {
            Statement::Declaration { body }
            | Statement::Block { body }
            | Statement::Labeled { body, .. }
            | Statement::With { body, .. }
            | Statement::While { body, .. }
            | Statement::DoWhile { body, .. } => {
                collect_eval_local_function_declarations_from_statements(
                    body,
                    local_function_names,
                    declarations,
                );
            }
            Statement::If {
                then_branch,
                else_branch,
                ..
            } => {
                collect_eval_local_function_declarations_from_statements(
                    then_branch,
                    local_function_names,
                    declarations,
                );
                collect_eval_local_function_declarations_from_statements(
                    else_branch,
                    local_function_names,
                    declarations,
                );
            }
            Statement::Try {
                body,
                catch_setup,
                catch_body,
                ..
            } => {
                collect_eval_local_function_declarations_from_statements(
                    body,
                    local_function_names,
                    declarations,
                );
                collect_eval_local_function_declarations_from_statements(
                    catch_setup,
                    local_function_names,
                    declarations,
                );
                collect_eval_local_function_declarations_from_statements(
                    catch_body,
                    local_function_names,
                    declarations,
                );
            }
            Statement::Switch { cases, .. } => {
                for case in cases {
                    collect_eval_local_function_declarations_from_statements(
                        &case.body,
                        local_function_names,
                        declarations,
                    );
                }
            }
            Statement::For { init, body, .. } => {
                collect_eval_local_function_declarations_from_statements(
                    init,
                    local_function_names,
                    declarations,
                );
                collect_eval_local_function_declarations_from_statements(
                    body,
                    local_function_names,
                    declarations,
                );
            }
            Statement::Var { .. }
            | Statement::Let { .. }
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

pub(in crate::backend::direct_wasm) fn eval_local_function_declaration_from_statement(
    statement: &Statement,
    local_function_names: &HashSet<String>,
) -> Option<(String, String)> {
    let Statement::Let { name, value, .. } = statement else {
        return None;
    };
    let Expression::Identifier(function_name) = value else {
        return None;
    };
    local_function_names
        .contains(function_name)
        .then(|| (name.clone(), function_name.clone()))
}
