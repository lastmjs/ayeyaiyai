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
            Statement::Block { body }
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
        Statement::Block { body }
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
            Statement::Block { body }
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

pub(in crate::backend::direct_wasm) fn scoped_binding_source_name(name: &str) -> Option<&str> {
    let rest = name.strip_prefix("__ayy_scope$")?;
    let (source_name, scope_id) = rest.rsplit_once('$')?;
    scope_id
        .chars()
        .all(|character| character.is_ascii_digit())
        .then_some(source_name)
}

pub(in crate::backend::direct_wasm) fn is_eval_local_function_declaration_statement(
    statement: &Statement,
    declarations: &HashMap<String, String>,
) -> bool {
    let Statement::Let { name, value, .. } = statement else {
        return false;
    };
    let Expression::Identifier(function_name) = value else {
        return false;
    };
    declarations
        .get(name)
        .is_some_and(|expected| expected == function_name)
}
