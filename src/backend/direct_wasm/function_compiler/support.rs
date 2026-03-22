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
    collect_loop_assigned_binding_names(
        condition.unwrap_or(&fallback_condition),
        break_hook,
        body,
        Some(init),
        update,
    )
}

pub(in crate::backend::direct_wasm) fn collect_eval_statement_var_names(
    statements: &[Statement],
) -> HashSet<String> {
    let mut names = HashSet::new();
    collect_eval_var_names_from_statements(statements, &mut names);
    names
}

pub(in crate::backend::direct_wasm) fn collect_referenced_binding_names_from_statements(
    statements: &[Statement],
) -> HashSet<String> {
    let mut names = HashSet::new();
    for statement in statements {
        collect_referenced_binding_names_from_statement(statement, &mut names);
    }
    names
}

pub(in crate::backend::direct_wasm) fn collect_assigned_binding_names_from_statement(
    statement: &Statement,
    names: &mut HashSet<String>,
) {
    match statement {
        Statement::Block { body } | Statement::Labeled { body, .. } => {
            for statement in body {
                collect_assigned_binding_names_from_statement(statement, names);
            }
        }
        Statement::Var { name, value } | Statement::Let { name, value, .. } => {
            names.insert(name.clone());
            collect_assigned_binding_names_from_expression(value, names);
        }
        Statement::Assign { name, value } => {
            names.insert(name.clone());
            collect_assigned_binding_names_from_expression(value, names);
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
        Statement::With { object, body } => {
            collect_assigned_binding_names_from_expression(object, names);
            for statement in body {
                collect_assigned_binding_names_from_statement(statement, names);
            }
        }
        Statement::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_assigned_binding_names_from_expression(condition, names);
            for statement in then_branch {
                collect_assigned_binding_names_from_statement(statement, names);
            }
            for statement in else_branch {
                collect_assigned_binding_names_from_statement(statement, names);
            }
        }
        Statement::Try {
            body,
            catch_setup,
            catch_body,
            ..
        } => {
            for statement in body {
                collect_assigned_binding_names_from_statement(statement, names);
            }
            for statement in catch_setup {
                collect_assigned_binding_names_from_statement(statement, names);
            }
            for statement in catch_body {
                collect_assigned_binding_names_from_statement(statement, names);
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
                    collect_assigned_binding_names_from_statement(statement, names);
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
                collect_assigned_binding_names_from_statement(statement, names);
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
                collect_assigned_binding_names_from_statement(statement, names);
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
                collect_assigned_binding_names_from_statement(statement, names);
            }
        }
        Statement::Break { .. } | Statement::Continue { .. } => {}
    }
}

pub(in crate::backend::direct_wasm) fn collect_assigned_binding_names_from_expression(
    expression: &Expression,
    names: &mut HashSet<String>,
) {
    match expression {
        Expression::Identifier(_)
        | Expression::Number(_)
        | Expression::BigInt(_)
        | Expression::String(_)
        | Expression::Bool(_)
        | Expression::Null
        | Expression::Undefined
        | Expression::This
        | Expression::NewTarget
        | Expression::Sent => {}
        Expression::Update { name, .. } => {
            names.insert(name.clone());
        }
        Expression::Member { object, property } => {
            collect_assigned_binding_names_from_expression(object, names);
            collect_assigned_binding_names_from_expression(property, names);
        }
        Expression::SuperMember { property } => {
            collect_assigned_binding_names_from_expression(property, names);
        }
        Expression::Assign { name, value } => {
            names.insert(name.clone());
            collect_assigned_binding_names_from_expression(value, names);
        }
        Expression::AssignMember {
            object,
            property,
            value,
        } => {
            collect_assigned_binding_names_from_expression(object, names);
            collect_assigned_binding_names_from_expression(property, names);
            collect_assigned_binding_names_from_expression(value, names);
        }
        Expression::AssignSuperMember { property, value } => {
            collect_assigned_binding_names_from_expression(property, names);
            collect_assigned_binding_names_from_expression(value, names);
        }
        Expression::Await(value)
        | Expression::EnumerateKeys(value)
        | Expression::GetIterator(value)
        | Expression::IteratorClose(value)
        | Expression::Unary {
            expression: value, ..
        } => collect_assigned_binding_names_from_expression(value, names),
        Expression::Binary { left, right, .. } => {
            collect_assigned_binding_names_from_expression(left, names);
            collect_assigned_binding_names_from_expression(right, names);
        }
        Expression::Conditional {
            condition,
            then_expression,
            else_expression,
        } => {
            collect_assigned_binding_names_from_expression(condition, names);
            collect_assigned_binding_names_from_expression(then_expression, names);
            collect_assigned_binding_names_from_expression(else_expression, names);
        }
        Expression::Sequence(expressions) => {
            for expression in expressions {
                collect_assigned_binding_names_from_expression(expression, names);
            }
        }
        Expression::Call { callee, arguments }
        | Expression::SuperCall { callee, arguments }
        | Expression::New { callee, arguments } => {
            collect_assigned_binding_names_from_expression(callee, names);
            for argument in arguments {
                match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                        collect_assigned_binding_names_from_expression(expression, names);
                    }
                }
            }
        }
        Expression::Array(elements) => {
            for element in elements {
                match element {
                    ArrayElement::Expression(expression) | ArrayElement::Spread(expression) => {
                        collect_assigned_binding_names_from_expression(expression, names);
                    }
                }
            }
        }
        Expression::Object(entries) => {
            for entry in entries {
                match entry {
                    ObjectEntry::Data { key, value } => {
                        collect_assigned_binding_names_from_expression(key, names);
                        collect_assigned_binding_names_from_expression(value, names);
                    }
                    ObjectEntry::Getter { key, getter } => {
                        collect_assigned_binding_names_from_expression(key, names);
                        collect_assigned_binding_names_from_expression(getter, names);
                    }
                    ObjectEntry::Setter { key, setter } => {
                        collect_assigned_binding_names_from_expression(key, names);
                        collect_assigned_binding_names_from_expression(setter, names);
                    }
                    ObjectEntry::Spread(expression) => {
                        collect_assigned_binding_names_from_expression(expression, names);
                    }
                }
            }
        }
    }
}

pub(in crate::backend::direct_wasm) fn collect_referenced_binding_names_from_statement(
    statement: &Statement,
    names: &mut HashSet<String>,
) {
    match statement {
        Statement::Block { body } | Statement::Labeled { body, .. } => {
            for statement in body {
                collect_referenced_binding_names_from_statement(statement, names);
            }
        }
        Statement::Var { value, .. }
        | Statement::Let { value, .. }
        | Statement::Expression(value)
        | Statement::Throw(value)
        | Statement::Return(value)
        | Statement::Yield { value }
        | Statement::YieldDelegate { value } => {
            collect_referenced_binding_names_from_expression(value, names);
        }
        Statement::Assign { name, value } => {
            names.insert(name.clone());
            collect_referenced_binding_names_from_expression(value, names);
        }
        Statement::AssignMember {
            object,
            property,
            value,
        } => {
            collect_referenced_binding_names_from_expression(object, names);
            collect_referenced_binding_names_from_expression(property, names);
            collect_referenced_binding_names_from_expression(value, names);
        }
        Statement::Print { values } => {
            for value in values {
                collect_referenced_binding_names_from_expression(value, names);
            }
        }
        Statement::With { object, body } => {
            collect_referenced_binding_names_from_expression(object, names);
            for statement in body {
                collect_referenced_binding_names_from_statement(statement, names);
            }
        }
        Statement::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_referenced_binding_names_from_expression(condition, names);
            for statement in then_branch {
                collect_referenced_binding_names_from_statement(statement, names);
            }
            for statement in else_branch {
                collect_referenced_binding_names_from_statement(statement, names);
            }
        }
        Statement::Try {
            body,
            catch_setup,
            catch_body,
            ..
        } => {
            for statement in body {
                collect_referenced_binding_names_from_statement(statement, names);
            }
            for statement in catch_setup {
                collect_referenced_binding_names_from_statement(statement, names);
            }
            for statement in catch_body {
                collect_referenced_binding_names_from_statement(statement, names);
            }
        }
        Statement::Switch {
            discriminant,
            cases,
            ..
        } => {
            collect_referenced_binding_names_from_expression(discriminant, names);
            for case in cases {
                if let Some(test) = &case.test {
                    collect_referenced_binding_names_from_expression(test, names);
                }
                for statement in &case.body {
                    collect_referenced_binding_names_from_statement(statement, names);
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
                collect_referenced_binding_names_from_statement(statement, names);
            }
            if let Some(condition) = condition {
                collect_referenced_binding_names_from_expression(condition, names);
            }
            if let Some(update) = update {
                collect_referenced_binding_names_from_expression(update, names);
            }
            if let Some(break_hook) = break_hook {
                collect_referenced_binding_names_from_expression(break_hook, names);
            }
            for statement in body {
                collect_referenced_binding_names_from_statement(statement, names);
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
            collect_referenced_binding_names_from_expression(condition, names);
            if let Some(break_hook) = break_hook {
                collect_referenced_binding_names_from_expression(break_hook, names);
            }
            for statement in body {
                collect_referenced_binding_names_from_statement(statement, names);
            }
        }
        Statement::Break { .. } | Statement::Continue { .. } => {}
    }
}

pub(in crate::backend::direct_wasm) fn collect_referenced_binding_names_from_expression(
    expression: &Expression,
    names: &mut HashSet<String>,
) {
    match expression {
        Expression::Identifier(name) | Expression::Update { name, .. } => {
            names.insert(name.clone());
        }
        Expression::Member { object, property } => {
            collect_referenced_binding_names_from_expression(object, names);
            collect_referenced_binding_names_from_expression(property, names);
        }
        Expression::SuperMember { property } => {
            collect_referenced_binding_names_from_expression(property, names);
        }
        Expression::Assign { name, value } => {
            names.insert(name.clone());
            collect_referenced_binding_names_from_expression(value, names);
        }
        Expression::AssignMember {
            object,
            property,
            value,
        } => {
            collect_referenced_binding_names_from_expression(object, names);
            collect_referenced_binding_names_from_expression(property, names);
            collect_referenced_binding_names_from_expression(value, names);
        }
        Expression::AssignSuperMember { property, value } => {
            collect_referenced_binding_names_from_expression(property, names);
            collect_referenced_binding_names_from_expression(value, names);
        }
        Expression::Await(value)
        | Expression::EnumerateKeys(value)
        | Expression::GetIterator(value)
        | Expression::IteratorClose(value)
        | Expression::Unary {
            expression: value, ..
        } => collect_referenced_binding_names_from_expression(value, names),
        Expression::Binary { left, right, .. } => {
            collect_referenced_binding_names_from_expression(left, names);
            collect_referenced_binding_names_from_expression(right, names);
        }
        Expression::Conditional {
            condition,
            then_expression,
            else_expression,
        } => {
            collect_referenced_binding_names_from_expression(condition, names);
            collect_referenced_binding_names_from_expression(then_expression, names);
            collect_referenced_binding_names_from_expression(else_expression, names);
        }
        Expression::Sequence(expressions) => {
            for expression in expressions {
                collect_referenced_binding_names_from_expression(expression, names);
            }
        }
        Expression::Call { callee, arguments }
        | Expression::SuperCall { callee, arguments }
        | Expression::New { callee, arguments } => {
            collect_referenced_binding_names_from_expression(callee, names);
            for argument in arguments {
                match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                        collect_referenced_binding_names_from_expression(expression, names);
                    }
                }
            }
        }
        Expression::Array(elements) => {
            for element in elements {
                match element {
                    ArrayElement::Expression(expression) | ArrayElement::Spread(expression) => {
                        collect_referenced_binding_names_from_expression(expression, names);
                    }
                }
            }
        }
        Expression::Object(entries) => {
            for entry in entries {
                match entry {
                    ObjectEntry::Data { key, value } => {
                        collect_referenced_binding_names_from_expression(key, names);
                        collect_referenced_binding_names_from_expression(value, names);
                    }
                    ObjectEntry::Getter { key, getter } => {
                        collect_referenced_binding_names_from_expression(key, names);
                        collect_referenced_binding_names_from_expression(getter, names);
                    }
                    ObjectEntry::Setter { key, setter } => {
                        collect_referenced_binding_names_from_expression(key, names);
                        collect_referenced_binding_names_from_expression(setter, names);
                    }
                    ObjectEntry::Spread(expression) => {
                        collect_referenced_binding_names_from_expression(expression, names);
                    }
                }
            }
        }
        Expression::Number(_)
        | Expression::BigInt(_)
        | Expression::String(_)
        | Expression::Bool(_)
        | Expression::Null
        | Expression::Undefined
        | Expression::This
        | Expression::NewTarget
        | Expression::Sent => {}
    }
}

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

pub(in crate::backend::direct_wasm) fn statement_references_user_function(
    statement: &Statement,
    names: &HashSet<String>,
) -> bool {
    match statement {
        Statement::Block { body } | Statement::Labeled { body, .. } => body
            .iter()
            .any(|statement| statement_references_user_function(statement, names)),
        Statement::Var { value, .. } | Statement::Let { value, .. } => {
            expression_references_user_function(value, names)
        }
        Statement::Assign { value, .. }
        | Statement::Expression(value)
        | Statement::Throw(value)
        | Statement::Return(value)
        | Statement::Yield { value }
        | Statement::YieldDelegate { value } => expression_references_user_function(value, names),
        Statement::AssignMember {
            object,
            property,
            value,
        } => {
            expression_references_user_function(object, names)
                || expression_references_user_function(property, names)
                || expression_references_user_function(value, names)
        }
        Statement::Print { values } => values
            .iter()
            .any(|value| expression_references_user_function(value, names)),
        Statement::With { object, body } => {
            expression_references_user_function(object, names)
                || body
                    .iter()
                    .any(|statement| statement_references_user_function(statement, names))
        }
        Statement::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expression_references_user_function(condition, names)
                || then_branch
                    .iter()
                    .any(|statement| statement_references_user_function(statement, names))
                || else_branch
                    .iter()
                    .any(|statement| statement_references_user_function(statement, names))
        }
        Statement::Try {
            body,
            catch_setup,
            catch_body,
            ..
        } => body
            .iter()
            .chain(catch_setup.iter())
            .chain(catch_body.iter())
            .any(|statement| statement_references_user_function(statement, names)),
        Statement::Switch {
            discriminant,
            cases,
            ..
        } => {
            expression_references_user_function(discriminant, names)
                || cases.iter().any(|case| {
                    case.test
                        .as_ref()
                        .is_some_and(|test| expression_references_user_function(test, names))
                        || case
                            .body
                            .iter()
                            .any(|statement| statement_references_user_function(statement, names))
                })
        }
        Statement::For {
            init,
            condition,
            update,
            break_hook,
            body,
            ..
        } => {
            init.iter()
                .any(|statement| statement_references_user_function(statement, names))
                || condition
                    .as_ref()
                    .is_some_and(|condition| expression_references_user_function(condition, names))
                || update
                    .as_ref()
                    .is_some_and(|update| expression_references_user_function(update, names))
                || break_hook.as_ref().is_some_and(|break_hook| {
                    expression_references_user_function(break_hook, names)
                })
                || body
                    .iter()
                    .any(|statement| statement_references_user_function(statement, names))
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
            expression_references_user_function(condition, names)
                || break_hook.as_ref().is_some_and(|break_hook| {
                    expression_references_user_function(break_hook, names)
                })
                || body
                    .iter()
                    .any(|statement| statement_references_user_function(statement, names))
        }
        Statement::Break { .. } | Statement::Continue { .. } => false,
    }
}

pub(in crate::backend::direct_wasm) fn expression_references_user_function(
    expression: &Expression,
    names: &HashSet<String>,
) -> bool {
    match expression {
        Expression::Identifier(name) => names.contains(name),
        Expression::Array(elements) => elements.iter().any(|element| match element {
            ArrayElement::Expression(expression) | ArrayElement::Spread(expression) => {
                expression_references_user_function(expression, names)
            }
        }),
        Expression::Object(entries) => entries.iter().any(|entry| match entry {
            ObjectEntry::Data { key, value } => {
                expression_references_user_function(key, names)
                    || expression_references_user_function(value, names)
            }
            ObjectEntry::Getter { key, getter } => {
                expression_references_user_function(key, names)
                    || expression_references_user_function(getter, names)
            }
            ObjectEntry::Setter { key, setter } => {
                expression_references_user_function(key, names)
                    || expression_references_user_function(setter, names)
            }
            ObjectEntry::Spread(expression) => {
                expression_references_user_function(expression, names)
            }
        }),
        Expression::Member { object, property } => {
            expression_references_user_function(object, names)
                || expression_references_user_function(property, names)
        }
        Expression::SuperMember { property } => {
            expression_references_user_function(property, names)
        }
        Expression::Assign { value, .. }
        | Expression::Await(value)
        | Expression::EnumerateKeys(value)
        | Expression::GetIterator(value)
        | Expression::IteratorClose(value)
        | Expression::Unary {
            expression: value, ..
        } => expression_references_user_function(value, names),
        Expression::AssignMember {
            object,
            property,
            value,
        } => {
            expression_references_user_function(object, names)
                || expression_references_user_function(property, names)
                || expression_references_user_function(value, names)
        }
        Expression::AssignSuperMember { property, value } => {
            expression_references_user_function(property, names)
                || expression_references_user_function(value, names)
        }
        Expression::Binary { left, right, .. } => {
            expression_references_user_function(left, names)
                || expression_references_user_function(right, names)
        }
        Expression::Conditional {
            condition,
            then_expression,
            else_expression,
        } => {
            expression_references_user_function(condition, names)
                || expression_references_user_function(then_expression, names)
                || expression_references_user_function(else_expression, names)
        }
        Expression::Sequence(expressions) => expressions
            .iter()
            .any(|expression| expression_references_user_function(expression, names)),
        Expression::Call { callee, arguments }
        | Expression::SuperCall { callee, arguments }
        | Expression::New { callee, arguments } => {
            expression_references_user_function(callee, names)
                || arguments.iter().any(|argument| match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                        expression_references_user_function(expression, names)
                    }
                })
        }
        Expression::Update { .. }
        | Expression::Number(_)
        | Expression::BigInt(_)
        | Expression::String(_)
        | Expression::Bool(_)
        | Expression::Null
        | Expression::Undefined
        | Expression::NewTarget
        | Expression::This
        | Expression::Sent => false,
    }
}

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
        Statement::Block { body } | Statement::Labeled { body, .. } => {
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

pub(in crate::backend::direct_wasm) fn collect_implicit_globals_from_expression(
    expression: &Expression,
    strict: bool,
    scope: &HashSet<String>,
    names: &mut BTreeSet<String>,
) -> DirectResult<()> {
    match expression {
        Expression::Assign { name, value } => {
            if !strict && !scope.contains(name) {
                names.insert(name.clone());
            }
            collect_implicit_globals_from_expression(value, strict, scope, names)
        }
        Expression::AssignMember {
            object,
            property,
            value,
        } => {
            collect_implicit_globals_from_expression(object, strict, scope, names)?;
            collect_implicit_globals_from_expression(property, strict, scope, names)?;
            collect_implicit_globals_from_expression(value, strict, scope, names)
        }
        Expression::AssignSuperMember { property, value } => {
            collect_implicit_globals_from_expression(property, strict, scope, names)?;
            collect_implicit_globals_from_expression(value, strict, scope, names)
        }
        Expression::Await(expression)
        | Expression::EnumerateKeys(expression)
        | Expression::GetIterator(expression)
        | Expression::IteratorClose(expression)
        | Expression::Unary { expression, .. } => {
            collect_implicit_globals_from_expression(expression, strict, scope, names)
        }
        Expression::Binary { left, right, .. } => {
            collect_implicit_globals_from_expression(left, strict, scope, names)?;
            collect_implicit_globals_from_expression(right, strict, scope, names)
        }
        Expression::Conditional {
            condition,
            then_expression,
            else_expression,
        } => {
            collect_implicit_globals_from_expression(condition, strict, scope, names)?;
            collect_implicit_globals_from_expression(then_expression, strict, scope, names)?;
            collect_implicit_globals_from_expression(else_expression, strict, scope, names)
        }
        Expression::Sequence(expressions) => {
            for expression in expressions {
                collect_implicit_globals_from_expression(expression, strict, scope, names)?;
            }
            Ok(())
        }
        Expression::Call { callee, arguments } | Expression::New { callee, arguments } => {
            collect_implicit_globals_from_expression(callee, strict, scope, names)?;
            for argument in arguments {
                match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                        collect_implicit_globals_from_expression(expression, strict, scope, names)?;
                    }
                }
            }

            if matches!(callee.as_ref(), Expression::Identifier(name) if name == "eval") {
                let Some(CallArgument::Expression(Expression::String(source))) = arguments.first()
                else {
                    return Ok(());
                };
                let eval_source = if strict {
                    format!("\"use strict\";{source}")
                } else {
                    source.clone()
                };
                if let Ok(program) = frontend::parse(&eval_source) {
                    let mut eval_scope = scope.clone();
                    eval_scope.extend(collect_declared_bindings_from_statements_recursive(
                        &program.statements,
                    ));
                    collect_implicit_globals_from_statements(
                        &program.statements,
                        program.strict,
                        &eval_scope,
                        names,
                    )?;
                }
            }

            Ok(())
        }
        Expression::SuperCall { callee, arguments } => {
            collect_implicit_globals_from_expression(callee, strict, scope, names)?;
            for argument in arguments {
                match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                        collect_implicit_globals_from_expression(expression, strict, scope, names)?;
                    }
                }
            }
            Ok(())
        }
        Expression::Array(elements) => {
            for element in elements {
                match element {
                    crate::ir::hir::ArrayElement::Expression(expression)
                    | crate::ir::hir::ArrayElement::Spread(expression) => {
                        collect_implicit_globals_from_expression(expression, strict, scope, names)?;
                    }
                }
            }
            Ok(())
        }
        Expression::Object(entries) => {
            for entry in entries {
                match entry {
                    crate::ir::hir::ObjectEntry::Data { key, value } => {
                        collect_implicit_globals_from_expression(key, strict, scope, names)?;
                        collect_implicit_globals_from_expression(value, strict, scope, names)?;
                    }
                    crate::ir::hir::ObjectEntry::Getter { key, getter } => {
                        collect_implicit_globals_from_expression(key, strict, scope, names)?;
                        collect_implicit_globals_from_expression(getter, strict, scope, names)?;
                    }
                    crate::ir::hir::ObjectEntry::Setter { key, setter } => {
                        collect_implicit_globals_from_expression(key, strict, scope, names)?;
                        collect_implicit_globals_from_expression(setter, strict, scope, names)?;
                    }
                    crate::ir::hir::ObjectEntry::Spread(expression) => {
                        collect_implicit_globals_from_expression(expression, strict, scope, names)?;
                    }
                }
            }
            Ok(())
        }
        Expression::Member { object, property } => {
            collect_implicit_globals_from_expression(object, strict, scope, names)?;
            collect_implicit_globals_from_expression(property, strict, scope, names)
        }
        Expression::SuperMember { property } => {
            collect_implicit_globals_from_expression(property, strict, scope, names)
        }
        Expression::Update { .. }
        | Expression::Number(_)
        | Expression::BigInt(_)
        | Expression::String(_)
        | Expression::Bool(_)
        | Expression::Null
        | Expression::Undefined
        | Expression::NewTarget
        | Expression::Identifier(_)
        | Expression::This
        | Expression::Sent => Ok(()),
    }
}

pub(in crate::backend::direct_wasm) fn user_function_runtime_value(
    user_function: &UserFunction,
) -> i32 {
    let offset = user_function
        .function_index
        .saturating_sub(USER_FUNCTION_BASE_INDEX);
    debug_assert!(offset < JS_USER_FUNCTION_VALUE_LIMIT as u32);
    JS_USER_FUNCTION_VALUE_BASE + offset as i32
}

pub(in crate::backend::direct_wasm) fn internal_function_name_hint(
    function_name: &str,
) -> Option<&str> {
    function_name
        .rsplit_once("__name_")
        .map(|(_, hinted_name)| hinted_name)
        .filter(|hinted_name| !hinted_name.is_empty())
}

pub(in crate::backend::direct_wasm) fn function_display_name(
    function: &FunctionDeclaration,
) -> Option<String> {
    function
        .self_binding
        .clone()
        .or_else(|| function.top_level_binding.clone())
        .or_else(|| internal_function_name_hint(&function.name).map(str::to_string))
        .or_else(|| (!function.name.starts_with("__ayy_")).then(|| function.name.clone()))
}

pub(in crate::backend::direct_wasm) fn builtin_function_display_name(name: &str) -> &str {
    match name {
        FUNCTION_CONSTRUCTOR_FAMILY_BUILTIN => "Function",
        _ => name,
    }
}

pub(in crate::backend::direct_wasm) fn builtin_function_runtime_value(name: &str) -> Option<i32> {
    match name {
        "eval" => Some(JS_BUILTIN_EVAL_VALUE),
        TEST262_CREATE_REALM_BUILTIN => Some(JS_TYPEOF_FUNCTION_TAG),
        _ => None,
    }
    .or_else(|| parse_test262_realm_eval_builtin(name).map(|_| JS_TYPEOF_FUNCTION_TAG))
}

pub(in crate::backend::direct_wasm) fn is_non_definable_global_name(name: &str) -> bool {
    matches!(name, "NaN" | "Infinity" | "undefined")
}

pub(in crate::backend::direct_wasm) fn is_reserved_js_runtime_value(integer: i64) -> bool {
    integer == JS_NULL_TAG as i64
        || integer == JS_UNDEFINED_TAG as i64
        || integer == JS_TYPEOF_NUMBER_TAG as i64
        || integer == JS_TYPEOF_STRING_TAG as i64
        || integer == JS_TYPEOF_BOOLEAN_TAG as i64
        || integer == JS_TYPEOF_OBJECT_TAG as i64
        || integer == JS_TYPEOF_UNDEFINED_TAG as i64
        || integer == JS_TYPEOF_FUNCTION_TAG as i64
        || integer == JS_TYPEOF_SYMBOL_TAG as i64
        || integer == JS_TYPEOF_BIGINT_TAG as i64
        || integer == JS_BUILTIN_EVAL_VALUE as i64
        || (integer >= JS_NATIVE_ERROR_VALUE_BASE as i64
            && integer < (JS_NATIVE_ERROR_VALUE_BASE + JS_NATIVE_ERROR_VALUE_LIMIT) as i64)
        || (integer >= JS_USER_FUNCTION_VALUE_BASE as i64
            && integer < (JS_USER_FUNCTION_VALUE_BASE + JS_USER_FUNCTION_VALUE_LIMIT) as i64)
}

pub(in crate::backend::direct_wasm) fn f64_to_i32(value: f64) -> DirectResult<i32> {
    if !value.is_finite() || value.fract() != 0.0 {
        return Ok(0);
    }

    let integer = value as i64;
    if integer < i32::MIN as i64 || integer > i32::MAX as i64 {
        return Ok(0);
    }
    if is_reserved_js_runtime_value(integer) {
        return Ok(0);
    }

    Ok(integer as i32)
}

pub(in crate::backend::direct_wasm) fn parse_bigint_to_i32(value: &str) -> DirectResult<i32> {
    let literal = value.strip_suffix('n').unwrap_or(value);
    let integer = literal.parse::<i64>().unwrap_or(0);

    if integer < i32::MIN as i64 || integer > i32::MAX as i64 {
        return Ok(0);
    }
    if is_reserved_js_runtime_value(integer) {
        return Ok(0);
    }

    Ok(integer as i32)
}

pub(in crate::backend::direct_wasm) fn parse_static_bigint_literal(
    value: &str,
) -> Option<StaticBigInt> {
    let literal = value.strip_suffix('n').unwrap_or(value);
    let (negative, magnitude) = if let Some(rest) = literal.strip_prefix('-') {
        (true, rest)
    } else if let Some(rest) = literal.strip_prefix('+') {
        (false, rest)
    } else {
        (false, literal)
    };
    let (radix, digits) = if let Some(rest) = magnitude
        .strip_prefix("0x")
        .or_else(|| magnitude.strip_prefix("0X"))
    {
        (16, rest)
    } else if let Some(rest) = magnitude
        .strip_prefix("0o")
        .or_else(|| magnitude.strip_prefix("0O"))
    {
        (8, rest)
    } else if let Some(rest) = magnitude
        .strip_prefix("0b")
        .or_else(|| magnitude.strip_prefix("0B"))
    {
        (2, rest)
    } else {
        (10, magnitude)
    };
    let parsed = StaticBigInt::parse_bytes(digits.as_bytes(), radix)?;
    Some(if negative { -parsed } else { parsed })
}

pub(in crate::backend::direct_wasm) fn parse_string_to_loose_i32(value: &str) -> DirectResult<i32> {
    if let Some(type_tag) = parse_typeof_tag_optional(value) {
        return Ok(type_tag);
    }

    parse_string_to_i32(value)
}

pub(in crate::backend::direct_wasm) fn parse_typeof_tag_optional(value: &str) -> Option<i32> {
    match parse_typeof_tag(value) {
        Ok(tag) => Some(tag),
        Err(_) => None,
    }
}

pub(in crate::backend::direct_wasm) fn parse_typeof_tag(value: &str) -> DirectResult<i32> {
    match value.trim() {
        "number" => Ok(JS_TYPEOF_NUMBER_TAG),
        "string" => Ok(JS_TYPEOF_STRING_TAG),
        "boolean" => Ok(JS_TYPEOF_BOOLEAN_TAG),
        "object" => Ok(JS_TYPEOF_OBJECT_TAG),
        "undefined" => Ok(JS_TYPEOF_UNDEFINED_TAG),
        "function" => Ok(JS_TYPEOF_FUNCTION_TAG),
        "symbol" => Ok(JS_TYPEOF_SYMBOL_TAG),
        "bigint" => Ok(JS_TYPEOF_BIGINT_TAG),
        _ => Ok(JS_TYPEOF_UNDEFINED_TAG),
    }
}

pub(in crate::backend::direct_wasm) fn parse_string_to_i32(value: &str) -> DirectResult<i32> {
    let trimmed = value.trim();

    let parsed = trimmed
        .parse::<i64>()
        .map_err(|_| Unsupported("non-numeric string literal"))?;

    if parsed < i32::MIN as i64 || parsed > i32::MAX as i64 {
        return Err(Unsupported("string literal integer is out of i32 range"));
    }
    if is_reserved_js_runtime_value(parsed) {
        return Err(Unsupported("string literal collides with reserved JS tag"));
    }

    Ok(parsed as i32)
}
