use super::*;

pub(in crate::backend::direct_wasm) fn collect_returned_identifier(
    statements: &[Statement],
) -> Option<String> {
    statements
        .iter()
        .rev()
        .find_map(collect_returned_identifier_from_statement)
}

pub(in crate::backend::direct_wasm) fn collect_returned_identifier_source_expression(
    statements: &[Statement],
) -> Option<Expression> {
    let returned_identifier = collect_returned_identifier(statements)?;
    statements.iter().rev().find_map(|statement| {
        collect_returned_identifier_source_expression_from_statement(
            statement,
            &returned_identifier,
        )
    })
}

pub(in crate::backend::direct_wasm) fn collect_returned_identifier_from_statement(
    statement: &Statement,
) -> Option<String> {
    match statement {
        Statement::Return(Expression::Identifier(name)) => Some(name.clone()),
        Statement::Block { body }
        | Statement::Labeled { body, .. }
        | Statement::With { body, .. } => collect_returned_identifier(body),
        Statement::If {
            then_branch,
            else_branch,
            ..
        } => collect_returned_identifier(then_branch)
            .or_else(|| collect_returned_identifier(else_branch)),
        Statement::Try {
            body,
            catch_setup,
            catch_body,
            ..
        } => collect_returned_identifier(body)
            .or_else(|| collect_returned_identifier(catch_setup))
            .or_else(|| collect_returned_identifier(catch_body)),
        Statement::Switch { cases, .. } => cases
            .iter()
            .rev()
            .find_map(|case| collect_returned_identifier(&case.body)),
        Statement::For { init, body, .. } => {
            collect_returned_identifier(body).or_else(|| collect_returned_identifier(init))
        }
        Statement::While { body, .. } | Statement::DoWhile { body, .. } => {
            collect_returned_identifier(body)
        }
        _ => None,
    }
}

pub(in crate::backend::direct_wasm) fn collect_returned_identifier_source_expression_from_statement(
    statement: &Statement,
    returned_identifier: &str,
) -> Option<Expression> {
    match statement {
        Statement::Var { name, value }
        | Statement::Let { name, value, .. }
        | Statement::Assign { name, value }
            if name == returned_identifier =>
        {
            Some(value.clone())
        }
        Statement::Block { body }
        | Statement::Labeled { body, .. }
        | Statement::With { body, .. } => collect_returned_identifier_source_expression(body),
        Statement::If {
            then_branch,
            else_branch,
            ..
        } => collect_returned_identifier_source_expression(then_branch)
            .or_else(|| collect_returned_identifier_source_expression(else_branch)),
        Statement::Try {
            body,
            catch_setup,
            catch_body,
            ..
        } => collect_returned_identifier_source_expression(body)
            .or_else(|| collect_returned_identifier_source_expression(catch_setup))
            .or_else(|| collect_returned_identifier_source_expression(catch_body)),
        Statement::Switch { cases, .. } => cases
            .iter()
            .rev()
            .find_map(|case| collect_returned_identifier_source_expression(&case.body)),
        Statement::For { init, body, .. } => collect_returned_identifier_source_expression(body)
            .or_else(|| collect_returned_identifier_source_expression(init)),
        Statement::While { body, .. } | Statement::DoWhile { body, .. } => {
            collect_returned_identifier_source_expression(body)
        }
        _ => None,
    }
}

pub(in crate::backend::direct_wasm) fn collect_returned_member_local_aliases(
    statements: &[Statement],
) -> HashMap<String, Expression> {
    let mut aliases = HashMap::new();
    for statement in statements {
        collect_returned_member_local_aliases_from_statement(statement, &mut aliases);
    }
    aliases
}

pub(in crate::backend::direct_wasm) fn collect_returned_member_local_aliases_from_statement(
    statement: &Statement,
    aliases: &mut HashMap<String, Expression>,
) {
    match statement {
        Statement::Block { body }
        | Statement::Labeled { body, .. }
        | Statement::With { body, .. } => {
            for statement in body {
                collect_returned_member_local_aliases_from_statement(statement, aliases);
            }
        }
        Statement::Var { name, value } | Statement::Let { name, value, .. } => {
            aliases.insert(
                name.clone(),
                resolve_returned_member_local_alias_expression(value, aliases),
            );
        }
        Statement::Assign { name, value } => {
            aliases.insert(
                name.clone(),
                resolve_returned_member_local_alias_expression(value, aliases),
            );
        }
        Statement::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_returned_member_local_aliases_from_expression(condition, aliases);
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
            collect_returned_member_local_aliases_from_expression(discriminant, aliases);
            for case in cases {
                if let Some(test) = &case.test {
                    collect_returned_member_local_aliases_from_expression(test, aliases);
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
                collect_returned_member_local_aliases_from_expression(condition, aliases);
            }
            if let Some(update) = update {
                collect_returned_member_local_aliases_from_expression(update, aliases);
            }
            if let Some(break_hook) = break_hook {
                collect_returned_member_local_aliases_from_expression(break_hook, aliases);
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
            collect_returned_member_local_aliases_from_expression(condition, aliases);
            if let Some(break_hook) = break_hook {
                collect_returned_member_local_aliases_from_expression(break_hook, aliases);
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
            collect_returned_member_local_aliases_from_expression(expression, aliases);
        }
        Statement::Print { values } => {
            for value in values {
                collect_returned_member_local_aliases_from_expression(value, aliases);
            }
        }
        Statement::AssignMember {
            object,
            property,
            value,
        } => {
            collect_returned_member_local_aliases_from_expression(object, aliases);
            collect_returned_member_local_aliases_from_expression(property, aliases);
            collect_returned_member_local_aliases_from_expression(value, aliases);
        }
        Statement::Break { .. } | Statement::Continue { .. } => {}
    }
}

pub(in crate::backend::direct_wasm) fn collect_returned_member_local_aliases_from_expression(
    expression: &Expression,
    aliases: &mut HashMap<String, Expression>,
) {
    match expression {
        Expression::Call { callee, arguments }
        | Expression::SuperCall { callee, arguments }
        | Expression::New { callee, arguments } => {
            collect_returned_member_local_aliases_from_expression(callee, aliases);
            for argument in arguments {
                match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                        collect_returned_member_local_aliases_from_expression(expression, aliases);
                    }
                }
            }
        }
        Expression::Unary { expression, .. }
        | Expression::Await(expression)
        | Expression::EnumerateKeys(expression)
        | Expression::GetIterator(expression)
        | Expression::IteratorClose(expression) => {
            collect_returned_member_local_aliases_from_expression(expression, aliases);
        }
        Expression::Binary { left, right, .. } => {
            collect_returned_member_local_aliases_from_expression(left, aliases);
            collect_returned_member_local_aliases_from_expression(right, aliases);
        }
        Expression::Conditional {
            condition,
            then_expression,
            else_expression,
        } => {
            collect_returned_member_local_aliases_from_expression(condition, aliases);
            collect_returned_member_local_aliases_from_expression(then_expression, aliases);
            collect_returned_member_local_aliases_from_expression(else_expression, aliases);
        }
        Expression::Sequence(expressions) => {
            for expression in expressions {
                collect_returned_member_local_aliases_from_expression(expression, aliases);
            }
        }
        Expression::Member { object, property } => {
            collect_returned_member_local_aliases_from_expression(object, aliases);
            collect_returned_member_local_aliases_from_expression(property, aliases);
        }
        Expression::Assign { value, .. } | Expression::AssignSuperMember { value, .. } => {
            collect_returned_member_local_aliases_from_expression(value, aliases);
        }
        Expression::AssignMember {
            object,
            property,
            value,
        } => {
            collect_returned_member_local_aliases_from_expression(object, aliases);
            collect_returned_member_local_aliases_from_expression(property, aliases);
            collect_returned_member_local_aliases_from_expression(value, aliases);
        }
        Expression::SuperMember { property } => {
            collect_returned_member_local_aliases_from_expression(property, aliases);
        }
        Expression::Array(elements) => {
            for element in elements {
                match element {
                    ArrayElement::Expression(expression) | ArrayElement::Spread(expression) => {
                        collect_returned_member_local_aliases_from_expression(expression, aliases);
                    }
                }
            }
        }
        Expression::Object(entries) => {
            for entry in entries {
                match entry {
                    ObjectEntry::Data { key, value } => {
                        collect_returned_member_local_aliases_from_expression(key, aliases);
                        collect_returned_member_local_aliases_from_expression(value, aliases);
                    }
                    ObjectEntry::Getter { key, getter } => {
                        collect_returned_member_local_aliases_from_expression(key, aliases);
                        collect_returned_member_local_aliases_from_expression(getter, aliases);
                    }
                    ObjectEntry::Setter { key, setter } => {
                        collect_returned_member_local_aliases_from_expression(key, aliases);
                        collect_returned_member_local_aliases_from_expression(setter, aliases);
                    }
                    ObjectEntry::Spread(value) => {
                        collect_returned_member_local_aliases_from_expression(value, aliases);
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
        | Expression::Identifier(_)
        | Expression::This
        | Expression::Sent
        | Expression::Update { .. } => {}
    }
}

pub(in crate::backend::direct_wasm) fn resolve_returned_member_local_alias_expression(
    expression: &Expression,
    aliases: &HashMap<String, Expression>,
) -> Expression {
    let mut current = expression;
    let mut visited = HashSet::new();
    loop {
        let Expression::Identifier(name) = current else {
            return current.clone();
        };
        if !visited.insert(name.clone()) {
            return expression.clone();
        }
        let Some(next) = aliases.get(name) else {
            return current.clone();
        };
        current = next;
    }
}

pub(in crate::backend::direct_wasm) fn collect_returned_object_literal(
    statements: &[Statement],
) -> Option<Vec<ObjectEntry>> {
    statements
        .iter()
        .rev()
        .find_map(collect_returned_object_literal_from_statement)
}

pub(in crate::backend::direct_wasm) fn collect_returned_object_literal_from_statement(
    statement: &Statement,
) -> Option<Vec<ObjectEntry>> {
    match statement {
        Statement::Return(Expression::Object(entries)) => Some(entries.clone()),
        Statement::Block { body }
        | Statement::Labeled { body, .. }
        | Statement::With { body, .. } => collect_returned_object_literal(body),
        Statement::If {
            then_branch,
            else_branch,
            ..
        } => collect_returned_object_literal(then_branch)
            .or_else(|| collect_returned_object_literal(else_branch)),
        Statement::Try {
            body,
            catch_setup,
            catch_body,
            ..
        } => collect_returned_object_literal(body)
            .or_else(|| collect_returned_object_literal(catch_setup))
            .or_else(|| collect_returned_object_literal(catch_body)),
        Statement::Switch { cases, .. } => cases
            .iter()
            .rev()
            .find_map(|case| collect_returned_object_literal(&case.body)),
        Statement::For { init, body, .. } => {
            collect_returned_object_literal(body).or_else(|| collect_returned_object_literal(init))
        }
        Statement::While { body, .. } | Statement::DoWhile { body, .. } => {
            collect_returned_object_literal(body)
        }
        _ => None,
    }
}

pub(in crate::backend::direct_wasm) fn collect_enumerated_keys_param_index(
    function: &FunctionDeclaration,
) -> Option<usize> {
    let returned_identifier = collect_returned_identifier(&function.body)?;
    let initialized_array = function.body.iter().any(|statement| {
        matches!(
            statement,
            Statement::Var { name, value }
                | Statement::Let { name, value, .. }
                | Statement::Assign { name, value }
                if name == &returned_identifier
                    && matches!(value, Expression::Array(elements) if elements.is_empty())
        )
    });
    if !initialized_array {
        return None;
    }

    function.body.iter().find_map(|statement| {
        match_enumerated_keys_collector_loop(statement, &returned_identifier, function)
    })
}

pub(in crate::backend::direct_wasm) fn match_enumerated_keys_collector_loop(
    statement: &Statement,
    returned_identifier: &str,
    function: &FunctionDeclaration,
) -> Option<usize> {
    let Statement::For {
        init,
        condition,
        update,
        body,
        ..
    } = statement
    else {
        return None;
    };

    let (target_name, param_index) = init.iter().find_map(|statement| match statement {
        Statement::Let { name, value, .. } | Statement::Var { name, value } => {
            let Expression::Identifier(param_name) = value else {
                return None;
            };
            function
                .params
                .iter()
                .position(|parameter| parameter.name == *param_name)
                .map(|param_index| (name.clone(), param_index))
        }
        _ => None,
    })?;

    let keys_name = init.iter().find_map(|statement| match statement {
        Statement::Let { name, value, .. } | Statement::Var { name, value } => {
            let Expression::EnumerateKeys(target) = value else {
                return None;
            };
            matches!(target.as_ref(), Expression::Identifier(current_target) if current_target == &target_name)
                .then(|| name.clone())
        }
        _ => None,
    })?;

    let index_name = match condition.as_ref()? {
        Expression::Binary {
            op: BinaryOp::LessThan,
            left,
            right,
        } => {
            let Expression::Identifier(index_name) = left.as_ref() else {
                return None;
            };
            matches!(
                right.as_ref(),
                Expression::Member { object, property }
                    if matches!(object.as_ref(), Expression::Identifier(name) if name == &keys_name)
                        && matches!(property.as_ref(), Expression::String(property_name) if property_name == "length")
            )
            .then(|| index_name.clone())?
        }
        _ => return None,
    };

    if !matches!(
        update.as_ref()?,
        Expression::Update {
            name,
            op: UpdateOp::Increment,
            ..
        } if name == &index_name
    ) {
        return None;
    }

    let loop_value_name = body.iter().find_map(|statement| match statement {
        Statement::Let { name, value, .. } | Statement::Var { name, value } => matches!(
            value,
            Expression::Member { object, property }
                if matches!(object.as_ref(), Expression::Identifier(current_keys) if current_keys == &keys_name)
                    && matches!(property.as_ref(), Expression::Identifier(current_index) if current_index == &index_name)
        )
        .then(|| name.clone()),
        _ => None,
    })?;

    body.iter().any(|statement| {
        matches!(
            statement,
            Statement::Expression(Expression::Call { callee, arguments })
                if matches!(
                    callee.as_ref(),
                    Expression::Member { object, property }
                        if matches!(object.as_ref(), Expression::Identifier(name) if name == returned_identifier)
                            && matches!(property.as_ref(), Expression::String(property_name) if property_name == "push")
                ) && matches!(
                    arguments.as_slice(),
                    [CallArgument::Expression(Expression::Identifier(argument_name))]
                        if argument_name == &loop_value_name
                )
        )
    })
    .then_some(param_index)
}
