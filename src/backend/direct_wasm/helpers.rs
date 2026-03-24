use super::*;

pub(in crate::backend::direct_wasm) fn empty_object_value_binding() -> ObjectValueBinding {
    ObjectValueBinding {
        string_properties: Vec::new(),
        symbol_properties: Vec::new(),
        non_enumerable_string_properties: Vec::new(),
    }
}

pub(in crate::backend::direct_wasm) fn test262_realm_identifier(id: u32) -> String {
    format!("{TEST262_REALM_IDENTIFIER_PREFIX}{id}")
}

pub(in crate::backend::direct_wasm) fn test262_realm_global_identifier(id: u32) -> String {
    format!("{TEST262_REALM_GLOBAL_IDENTIFIER_PREFIX}{id}")
}

pub(in crate::backend::direct_wasm) fn test262_realm_eval_builtin_name(id: u32) -> String {
    format!("{TEST262_REALM_EVAL_BUILTIN_PREFIX}{id}")
}

fn stable_eval_namespace_hash(text: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in text.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn eval_program_function_namespace(current_function_name: Option<&str>, source: &str) -> String {
    let context_hash = stable_eval_namespace_hash(current_function_name.unwrap_or("__top__"));
    let source_hash = stable_eval_namespace_hash(source);
    format!("__evalctx_{context_hash:016x}_{source_hash:016x}")
}

pub(in crate::backend::direct_wasm) fn namespaced_internal_eval_function_name(
    function_name: &str,
    namespace: &str,
) -> String {
    if let Some((prefix, hint)) = function_name.rsplit_once("__name_") {
        return format!("{prefix}__{namespace}__name_{hint}");
    }
    format!("{function_name}__{namespace}")
}

fn rewrite_eval_program_internal_function_name(
    name: &mut String,
    rename_map: &HashMap<String, String>,
) {
    if let Some(renamed) = rename_map.get(name) {
        *name = renamed.clone();
    }
}

fn rewrite_eval_program_internal_function_names_in_expression(
    expression: &mut Expression,
    rename_map: &HashMap<String, String>,
) {
    match expression {
        Expression::Array(elements) => {
            for element in elements {
                match element {
                    ArrayElement::Expression(expression) | ArrayElement::Spread(expression) => {
                        rewrite_eval_program_internal_function_names_in_expression(
                            expression, rename_map,
                        );
                    }
                }
            }
        }
        Expression::Object(entries) => {
            for entry in entries {
                match entry {
                    ObjectEntry::Data { key, value } => {
                        rewrite_eval_program_internal_function_names_in_expression(key, rename_map);
                        rewrite_eval_program_internal_function_names_in_expression(
                            value, rename_map,
                        );
                    }
                    ObjectEntry::Getter { key, getter } => {
                        rewrite_eval_program_internal_function_names_in_expression(key, rename_map);
                        rewrite_eval_program_internal_function_names_in_expression(
                            getter, rename_map,
                        );
                    }
                    ObjectEntry::Setter { key, setter } => {
                        rewrite_eval_program_internal_function_names_in_expression(key, rename_map);
                        rewrite_eval_program_internal_function_names_in_expression(
                            setter, rename_map,
                        );
                    }
                    ObjectEntry::Spread(expression) => {
                        rewrite_eval_program_internal_function_names_in_expression(
                            expression, rename_map,
                        );
                    }
                }
            }
        }
        Expression::Identifier(name) | Expression::Update { name, .. } => {
            rewrite_eval_program_internal_function_name(name, rename_map);
        }
        Expression::Member { object, property } => {
            rewrite_eval_program_internal_function_names_in_expression(object, rename_map);
            rewrite_eval_program_internal_function_names_in_expression(property, rename_map);
        }
        Expression::SuperMember { property }
        | Expression::Await(property)
        | Expression::EnumerateKeys(property)
        | Expression::GetIterator(property)
        | Expression::IteratorClose(property)
        | Expression::Unary {
            expression: property,
            ..
        } => rewrite_eval_program_internal_function_names_in_expression(property, rename_map),
        Expression::Assign { name, value } => {
            rewrite_eval_program_internal_function_name(name, rename_map);
            rewrite_eval_program_internal_function_names_in_expression(value, rename_map);
        }
        Expression::AssignMember {
            object,
            property,
            value,
        } => {
            rewrite_eval_program_internal_function_names_in_expression(object, rename_map);
            rewrite_eval_program_internal_function_names_in_expression(property, rename_map);
            rewrite_eval_program_internal_function_names_in_expression(value, rename_map);
        }
        Expression::AssignSuperMember { property, value } => {
            rewrite_eval_program_internal_function_names_in_expression(property, rename_map);
            rewrite_eval_program_internal_function_names_in_expression(value, rename_map);
        }
        Expression::Binary { left, right, .. } => {
            rewrite_eval_program_internal_function_names_in_expression(left, rename_map);
            rewrite_eval_program_internal_function_names_in_expression(right, rename_map);
        }
        Expression::Conditional {
            condition,
            then_expression,
            else_expression,
        } => {
            rewrite_eval_program_internal_function_names_in_expression(condition, rename_map);
            rewrite_eval_program_internal_function_names_in_expression(then_expression, rename_map);
            rewrite_eval_program_internal_function_names_in_expression(else_expression, rename_map);
        }
        Expression::Sequence(expressions) => {
            for expression in expressions {
                rewrite_eval_program_internal_function_names_in_expression(expression, rename_map);
            }
        }
        Expression::Call { callee, arguments }
        | Expression::SuperCall { callee, arguments }
        | Expression::New { callee, arguments } => {
            rewrite_eval_program_internal_function_names_in_expression(callee, rename_map);
            for argument in arguments {
                match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                        rewrite_eval_program_internal_function_names_in_expression(
                            expression, rename_map,
                        );
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
        | Expression::NewTarget
        | Expression::This
        | Expression::Sent => {}
    }
}

fn rewrite_eval_program_internal_function_names_in_statement(
    statement: &mut Statement,
    rename_map: &HashMap<String, String>,
) {
    match statement {
        Statement::Block { body } | Statement::Labeled { body, .. } => {
            for statement in body {
                rewrite_eval_program_internal_function_names_in_statement(statement, rename_map);
            }
        }
        Statement::Var { name, value } | Statement::Let { name, value, .. } => {
            rewrite_eval_program_internal_function_name(name, rename_map);
            rewrite_eval_program_internal_function_names_in_expression(value, rename_map);
        }
        Statement::Assign { name, value } => {
            rewrite_eval_program_internal_function_name(name, rename_map);
            rewrite_eval_program_internal_function_names_in_expression(value, rename_map);
        }
        Statement::AssignMember {
            object,
            property,
            value,
        } => {
            rewrite_eval_program_internal_function_names_in_expression(object, rename_map);
            rewrite_eval_program_internal_function_names_in_expression(property, rename_map);
            rewrite_eval_program_internal_function_names_in_expression(value, rename_map);
        }
        Statement::Print { values } => {
            for value in values {
                rewrite_eval_program_internal_function_names_in_expression(value, rename_map);
            }
        }
        Statement::Expression(expression)
        | Statement::Throw(expression)
        | Statement::Return(expression)
        | Statement::Yield { value: expression }
        | Statement::YieldDelegate { value: expression } => {
            rewrite_eval_program_internal_function_names_in_expression(expression, rename_map);
        }
        Statement::With { object, body } => {
            rewrite_eval_program_internal_function_names_in_expression(object, rename_map);
            for statement in body {
                rewrite_eval_program_internal_function_names_in_statement(statement, rename_map);
            }
        }
        Statement::If {
            condition,
            then_branch,
            else_branch,
        } => {
            rewrite_eval_program_internal_function_names_in_expression(condition, rename_map);
            for statement in then_branch {
                rewrite_eval_program_internal_function_names_in_statement(statement, rename_map);
            }
            for statement in else_branch {
                rewrite_eval_program_internal_function_names_in_statement(statement, rename_map);
            }
        }
        Statement::Try {
            catch_binding,
            body,
            catch_setup,
            catch_body,
        } => {
            if let Some(catch_binding) = catch_binding {
                rewrite_eval_program_internal_function_name(catch_binding, rename_map);
            }
            for statement in body {
                rewrite_eval_program_internal_function_names_in_statement(statement, rename_map);
            }
            for statement in catch_setup {
                rewrite_eval_program_internal_function_names_in_statement(statement, rename_map);
            }
            for statement in catch_body {
                rewrite_eval_program_internal_function_names_in_statement(statement, rename_map);
            }
        }
        Statement::Switch {
            bindings,
            discriminant,
            cases,
            ..
        } => {
            for binding in bindings {
                rewrite_eval_program_internal_function_name(binding, rename_map);
            }
            rewrite_eval_program_internal_function_names_in_expression(discriminant, rename_map);
            for case in cases {
                if let Some(test) = &mut case.test {
                    rewrite_eval_program_internal_function_names_in_expression(test, rename_map);
                }
                for statement in &mut case.body {
                    rewrite_eval_program_internal_function_names_in_statement(
                        statement, rename_map,
                    );
                }
            }
        }
        Statement::For {
            init,
            per_iteration_bindings,
            condition,
            update,
            break_hook,
            body,
            ..
        } => {
            for statement in init {
                rewrite_eval_program_internal_function_names_in_statement(statement, rename_map);
            }
            for binding in per_iteration_bindings {
                rewrite_eval_program_internal_function_name(binding, rename_map);
            }
            if let Some(condition) = condition {
                rewrite_eval_program_internal_function_names_in_expression(condition, rename_map);
            }
            if let Some(update) = update {
                rewrite_eval_program_internal_function_names_in_expression(update, rename_map);
            }
            if let Some(break_hook) = break_hook {
                rewrite_eval_program_internal_function_names_in_expression(break_hook, rename_map);
            }
            for statement in body {
                rewrite_eval_program_internal_function_names_in_statement(statement, rename_map);
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
            rewrite_eval_program_internal_function_names_in_expression(condition, rename_map);
            if let Some(break_hook) = break_hook {
                rewrite_eval_program_internal_function_names_in_expression(break_hook, rename_map);
            }
            for statement in body {
                rewrite_eval_program_internal_function_names_in_statement(statement, rename_map);
            }
        }
        Statement::Break { .. } | Statement::Continue { .. } => {}
    }
}

pub(in crate::backend::direct_wasm) fn namespace_eval_program_internal_function_names(
    program: &mut Program,
    current_function_name: Option<&str>,
    source: &str,
) {
    let namespace = eval_program_function_namespace(current_function_name, source);
    let rename_map = program
        .functions
        .iter()
        .filter(|function| is_internal_user_function_identifier(&function.name))
        .map(|function| {
            (
                function.name.clone(),
                namespaced_internal_eval_function_name(&function.name, &namespace),
            )
        })
        .collect::<HashMap<_, _>>();
    if rename_map.is_empty() {
        return;
    }

    for statement in &mut program.statements {
        rewrite_eval_program_internal_function_names_in_statement(statement, &rename_map);
    }
    for function in &mut program.functions {
        rewrite_eval_program_internal_function_name(&mut function.name, &rename_map);
        if let Some(binding) = &mut function.top_level_binding {
            rewrite_eval_program_internal_function_name(binding, &rename_map);
        }
        if let Some(binding) = &mut function.self_binding {
            rewrite_eval_program_internal_function_name(binding, &rename_map);
        }
        for parameter in &mut function.params {
            rewrite_eval_program_internal_function_name(&mut parameter.name, &rename_map);
            if let Some(default) = &mut parameter.default {
                rewrite_eval_program_internal_function_names_in_expression(default, &rename_map);
            }
        }
        for statement in &mut function.body {
            rewrite_eval_program_internal_function_names_in_statement(statement, &rename_map);
        }
    }
}

fn parse_prefixed_u32(name: &str, prefix: &str) -> Option<u32> {
    name.strip_prefix(prefix)?.parse::<u32>().ok()
}

pub(in crate::backend::direct_wasm) fn parse_test262_realm_identifier(name: &str) -> Option<u32> {
    parse_prefixed_u32(name, TEST262_REALM_IDENTIFIER_PREFIX)
}

pub(in crate::backend::direct_wasm) fn parse_test262_realm_global_identifier(
    name: &str,
) -> Option<u32> {
    parse_prefixed_u32(name, TEST262_REALM_GLOBAL_IDENTIFIER_PREFIX)
}

pub(in crate::backend::direct_wasm) fn parse_test262_realm_eval_builtin(name: &str) -> Option<u32> {
    parse_prefixed_u32(name, TEST262_REALM_EVAL_BUILTIN_PREFIX)
}
