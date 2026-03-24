use super::*;

pub(in crate::backend::direct_wasm) fn collect_function_constructor_local_bindings(
    function: &FunctionDeclaration,
) -> HashSet<String> {
    let mut bindings = collect_declared_bindings_from_statements_recursive(&function.body);
    bindings.extend(
        function
            .params
            .iter()
            .map(|parameter| parameter.name.clone()),
    );
    if let Some(self_binding) = &function.self_binding {
        bindings.insert(self_binding.clone());
    }
    bindings.insert("arguments".to_string());
    bindings
}

pub(in crate::backend::direct_wasm) fn builtin_identifier_delete_returns_true(name: &str) -> bool {
    builtin_identifier_kind(name).is_some() && !matches!(name, "Infinity" | "NaN" | "undefined")
}

pub(in crate::backend::direct_wasm) fn builtin_member_delete_returns_false(
    object_name: &str,
    property_name: &str,
) -> bool {
    object_name == "Math" && builtin_member_number_value(object_name, property_name).is_some()
}

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
        Statement::Block { body } | Statement::Labeled { body, .. } => {
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

pub(in crate::backend::direct_wasm) fn eval_program_declares_var_arguments(
    program: &Program,
) -> bool {
    eval_statements_declare_var_arguments(&program.statements)
}

pub(in crate::backend::direct_wasm) fn collect_direct_eval_lexical_binding_names(
    statements: &[Statement],
) -> Vec<String> {
    let mut bindings = Vec::new();
    let mut seen = HashSet::new();
    for statement in statements {
        if let Statement::Let { name, .. } = statement {
            if seen.insert(name.clone()) {
                bindings.push(name.clone());
            }
        }
    }
    bindings
}

pub(in crate::backend::direct_wasm) fn collect_loop_assigned_binding_names(
    condition: &Expression,
    break_hook: Option<&Expression>,
    body: &[Statement],
    init: Option<&[Statement]>,
    update: Option<&Expression>,
) -> HashSet<String> {
    let mut names = HashSet::new();
    if let Some(init) = init {
        for statement in init {
            collect_assigned_binding_names_from_statement(statement, &mut names);
        }
    }
    collect_assigned_binding_names_from_expression(condition, &mut names);
    if let Some(update) = update {
        collect_assigned_binding_names_from_expression(update, &mut names);
    }
    if let Some(break_hook) = break_hook {
        collect_assigned_binding_names_from_expression(break_hook, &mut names);
    }
    for statement in body {
        collect_assigned_binding_names_from_statement(statement, &mut names);
    }
    names
}

pub(in crate::backend::direct_wasm) fn collect_loop_assigned_binding_names_from_for(
    init: &[Statement],
    condition: Option<&Expression>,
    update: Option<&Expression>,
    break_hook: Option<&Expression>,
    body: &[Statement],
) -> HashSet<String> {
    let fallback_condition = Expression::Bool(true);
    let mut names = collect_loop_assigned_binding_names(
        condition.unwrap_or(&fallback_condition),
        break_hook,
        body,
        None,
        update,
    );
    for statement in init {
        collect_assigned_binding_names_from_expression_in_loop_initializer(statement, &mut names);
    }
    names
}

pub(in crate::backend::direct_wasm) fn collect_assigned_binding_names_from_expression_in_loop_initializer(
    statement: &Statement,
    names: &mut HashSet<String>,
) {
    match statement {
        Statement::Var { value, .. } | Statement::Let { value, .. } => {
            collect_assigned_binding_names_from_expression(value, names);
        }
        Statement::Assign { value, .. } => {
            collect_assigned_binding_names_from_expression(value, names);
        }
        Statement::Block { body } | Statement::Labeled { body, .. } => {
            for statement in body {
                collect_assigned_binding_names_from_expression_in_loop_initializer(
                    statement, names,
                );
            }
        }
        Statement::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_assigned_binding_names_from_expression(condition, names);
            for statement in then_branch {
                collect_assigned_binding_names_from_expression_in_loop_initializer(
                    statement, names,
                );
            }
            for statement in else_branch {
                collect_assigned_binding_names_from_expression_in_loop_initializer(
                    statement, names,
                );
            }
        }
        Statement::With { object, body } => {
            collect_assigned_binding_names_from_expression(object, names);
            for statement in body {
                collect_assigned_binding_names_from_expression_in_loop_initializer(
                    statement, names,
                );
            }
        }
        Statement::Expression(expression)
        | Statement::Throw(expression)
        | Statement::Return(expression)
        | Statement::Yield { value: expression }
        | Statement::YieldDelegate { value: expression } => {
            collect_assigned_binding_names_from_expression(expression, names);
        }
        Statement::Print { values } => {
            for value in values {
                collect_assigned_binding_names_from_expression(value, names);
            }
        }
        Statement::AssignMember {
            object,
            property,
            value,
        } => {
            collect_assigned_binding_names_from_expression(object, names);
            collect_assigned_binding_names_from_expression(property, names);
            collect_assigned_binding_names_from_expression(value, names);
        }
        Statement::Try {
            body,
            catch_setup,
            catch_body,
            ..
        } => {
            for statement in body {
                collect_assigned_binding_names_from_expression_in_loop_initializer(
                    statement, names,
                );
            }
            for statement in catch_setup {
                collect_assigned_binding_names_from_expression_in_loop_initializer(
                    statement, names,
                );
            }
            for statement in catch_body {
                collect_assigned_binding_names_from_expression_in_loop_initializer(
                    statement, names,
                );
            }
        }
        Statement::Switch {
            discriminant,
            cases,
            ..
        } => {
            collect_assigned_binding_names_from_expression(discriminant, names);
            for case in cases {
                if let Some(test) = &case.test {
                    collect_assigned_binding_names_from_expression(test, names);
                }
                for statement in &case.body {
                    collect_assigned_binding_names_from_expression_in_loop_initializer(
                        statement, names,
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
                collect_assigned_binding_names_from_expression_in_loop_initializer(
                    statement, names,
                );
            }
            if let Some(condition) = condition {
                collect_assigned_binding_names_from_expression(condition, names);
            }
            if let Some(update) = update {
                collect_assigned_binding_names_from_expression(update, names);
            }
            if let Some(break_hook) = break_hook {
                collect_assigned_binding_names_from_expression(break_hook, names);
            }
            for statement in body {
                collect_assigned_binding_names_from_expression_in_loop_initializer(
                    statement, names,
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
            collect_assigned_binding_names_from_expression(condition, names);
            if let Some(break_hook) = break_hook {
                collect_assigned_binding_names_from_expression(break_hook, names);
            }
            for statement in body {
                collect_assigned_binding_names_from_expression_in_loop_initializer(
                    statement, names,
                );
            }
        }
        Statement::Break { .. } | Statement::Continue { .. } => {}
    }
}
