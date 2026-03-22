use super::*;

pub(in crate::backend::direct_wasm) fn collect_arguments_usage_from_statements(
    statements: &[Statement],
) -> ArgumentsUsage {
    let mut indexed_slots = HashSet::new();
    let mut track_all_slots = false;
    for statement in statements {
        collect_arguments_usage_from_statement(statement, &mut indexed_slots, &mut track_all_slots);
    }
    if track_all_slots {
        indexed_slots.extend(0..TRACKED_ARGUMENT_SLOT_LIMIT);
    }
    let mut indexed_slots = indexed_slots.into_iter().collect::<Vec<_>>();
    indexed_slots.sort_unstable();
    ArgumentsUsage { indexed_slots }
}

pub(in crate::backend::direct_wasm) fn function_returns_arguments_object(
    statements: &[Statement],
) -> bool {
    statements.iter().any(statement_returns_arguments_object)
}

pub(in crate::backend::direct_wasm) fn collect_returned_arguments_effects(
    statements: &[Statement],
) -> ReturnedArgumentsEffects {
    let mut effects = ReturnedArgumentsEffects::default();
    for statement in statements {
        collect_returned_arguments_effects_from_statement(statement, &mut effects);
    }
    effects
}

pub(in crate::backend::direct_wasm) fn statement_returns_arguments_object(
    statement: &Statement,
) -> bool {
    match statement {
        Statement::Return(Expression::Identifier(name)) => name == "arguments",
        Statement::Block { body } | Statement::Labeled { body, .. } => {
            body.iter().any(statement_returns_arguments_object)
        }
        Statement::If {
            then_branch,
            else_branch,
            ..
        } => {
            then_branch.iter().any(statement_returns_arguments_object)
                || else_branch.iter().any(statement_returns_arguments_object)
        }
        Statement::Try {
            body,
            catch_setup,
            catch_body,
            ..
        } => {
            body.iter().any(statement_returns_arguments_object)
                || catch_setup.iter().any(statement_returns_arguments_object)
                || catch_body.iter().any(statement_returns_arguments_object)
        }
        Statement::Switch { cases, .. } => cases
            .iter()
            .any(|case| case.body.iter().any(statement_returns_arguments_object)),
        Statement::For { init, body, .. } => {
            init.iter().any(statement_returns_arguments_object)
                || body.iter().any(statement_returns_arguments_object)
        }
        Statement::While { body, .. } | Statement::DoWhile { body, .. } => {
            body.iter().any(statement_returns_arguments_object)
        }
        _ => false,
    }
}

pub(in crate::backend::direct_wasm) fn collect_returned_arguments_effects_from_statement(
    statement: &Statement,
    effects: &mut ReturnedArgumentsEffects,
) {
    match statement {
        Statement::Block { body } | Statement::Labeled { body, .. } => {
            for statement in body {
                collect_returned_arguments_effects_from_statement(statement, effects);
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
            collect_returned_arguments_effects_from_expression(value, effects);
        }
        Statement::AssignMember {
            object,
            property,
            value,
        } => {
            collect_returned_arguments_effects_from_expression(value, effects);
            if let Some(property_name) = direct_arguments_named_property(object, property) {
                let effect = ArgumentsPropertyEffect::Assign(value.clone());
                match property_name {
                    "callee" => effects.callee = Some(effect),
                    "length" => effects.length = Some(effect),
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

pub(in crate::backend::direct_wasm) fn collect_returned_arguments_effects_from_expression(
    expression: &Expression,
    effects: &mut ReturnedArgumentsEffects,
) {
    match expression {
        Expression::AssignMember {
            object,
            property,
            value,
        } => {
            collect_returned_arguments_effects_from_expression(value, effects);
            if let Some(property_name) = direct_arguments_named_property(object, property) {
                let effect = ArgumentsPropertyEffect::Assign((**value).clone());
                match property_name {
                    "callee" => effects.callee = Some(effect),
                    "length" => effects.length = Some(effect),
                    _ => {}
                }
            }
        }
        Expression::Unary {
            op: UnaryOp::Delete,
            expression,
        } => {
            if let Expression::Member { object, property } = expression.as_ref() {
                if let Some(property_name) = direct_arguments_named_property(object, property) {
                    match property_name {
                        "callee" => effects.callee = Some(ArgumentsPropertyEffect::Delete),
                        "length" => effects.length = Some(ArgumentsPropertyEffect::Delete),
                        _ => {}
                    }
                }
            }
        }
        Expression::Sequence(expressions) => {
            for expression in expressions {
                collect_returned_arguments_effects_from_expression(expression, effects);
            }
        }
        Expression::Binary { left, right, .. } => {
            collect_returned_arguments_effects_from_expression(left, effects);
            collect_returned_arguments_effects_from_expression(right, effects);
        }
        Expression::Conditional {
            condition,
            then_expression,
            else_expression,
        } => {
            collect_returned_arguments_effects_from_expression(condition, effects);
            collect_returned_arguments_effects_from_expression(then_expression, effects);
            collect_returned_arguments_effects_from_expression(else_expression, effects);
        }
        Expression::Call { callee, arguments }
        | Expression::SuperCall { callee, arguments }
        | Expression::New { callee, arguments } => {
            collect_returned_arguments_effects_from_expression(callee, effects);
            for argument in arguments {
                match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                        collect_returned_arguments_effects_from_expression(expression, effects);
                    }
                }
            }
        }
        Expression::Member { object, property } => {
            collect_returned_arguments_effects_from_expression(object, effects);
            collect_returned_arguments_effects_from_expression(property, effects);
        }
        Expression::Assign { value, .. }
        | Expression::AssignSuperMember { value, .. }
        | Expression::Await(value)
        | Expression::EnumerateKeys(value)
        | Expression::GetIterator(value)
        | Expression::IteratorClose(value)
        | Expression::Unary {
            expression: value, ..
        } => {
            collect_returned_arguments_effects_from_expression(value, effects);
        }
        Expression::SuperMember { property } => {
            collect_returned_arguments_effects_from_expression(property, effects);
        }
        _ => {}
    }
}

pub(in crate::backend::direct_wasm) fn direct_arguments_named_property(
    object: &Expression,
    property: &Expression,
) -> Option<&'static str> {
    if !is_arguments_identifier(object) {
        return None;
    }
    match property {
        Expression::String(property_name) if property_name == "callee" => Some("callee"),
        Expression::String(property_name) if property_name == "length" => Some("length"),
        _ => None,
    }
}

pub(in crate::backend::direct_wasm) fn collect_arguments_usage_from_statement(
    statement: &Statement,
    indexed_slots: &mut HashSet<u32>,
    track_all_slots: &mut bool,
) {
    match statement {
        Statement::Block { body } | Statement::Labeled { body, .. } => {
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

pub(in crate::backend::direct_wasm) fn collect_arguments_usage_from_expression(
    expression: &Expression,
    indexed_slots: &mut HashSet<u32>,
    track_all_slots: &mut bool,
) {
    match expression {
        Expression::Member { object, property } => {
            if is_arguments_identifier(object) {
                if let Some(index) = argument_index_from_expression(property) {
                    indexed_slots.insert(index);
                } else {
                    *track_all_slots = true;
                }
            }
            collect_arguments_usage_from_expression(object, indexed_slots, track_all_slots);
            collect_arguments_usage_from_expression(property, indexed_slots, track_all_slots);
        }
        Expression::AssignMember {
            object,
            property,
            value,
        } => {
            if is_arguments_identifier(object) {
                if let Some(index) = argument_index_from_expression(property) {
                    indexed_slots.insert(index);
                } else {
                    *track_all_slots = true;
                }
            }
            collect_arguments_usage_from_expression(object, indexed_slots, track_all_slots);
            collect_arguments_usage_from_expression(property, indexed_slots, track_all_slots);
            collect_arguments_usage_from_expression(value, indexed_slots, track_all_slots);
        }
        Expression::Assign { value, .. }
        | Expression::Await(value)
        | Expression::EnumerateKeys(value)
        | Expression::IteratorClose(value) => {
            collect_arguments_usage_from_expression(value, indexed_slots, track_all_slots);
        }
        Expression::GetIterator(value) => {
            if is_arguments_identifier(value) {
                *track_all_slots = true;
            }
            collect_arguments_usage_from_expression(value, indexed_slots, track_all_slots);
        }
        Expression::Unary { op, expression } => {
            if *op == UnaryOp::Delete {
                if let Expression::Member { object, property } = expression.as_ref() {
                    if is_arguments_identifier(object) {
                        if let Some(index) = argument_index_from_expression(property) {
                            indexed_slots.insert(index);
                        } else {
                            *track_all_slots = true;
                        }
                    }
                }
            }
            collect_arguments_usage_from_expression(expression, indexed_slots, track_all_slots);
        }
        Expression::Binary { left, right, .. } => {
            collect_arguments_usage_from_expression(left, indexed_slots, track_all_slots);
            collect_arguments_usage_from_expression(right, indexed_slots, track_all_slots);
        }
        Expression::Conditional {
            condition,
            then_expression,
            else_expression,
        } => {
            collect_arguments_usage_from_expression(condition, indexed_slots, track_all_slots);
            collect_arguments_usage_from_expression(
                then_expression,
                indexed_slots,
                track_all_slots,
            );
            collect_arguments_usage_from_expression(
                else_expression,
                indexed_slots,
                track_all_slots,
            );
        }
        Expression::Sequence(expressions) => {
            for expression in expressions {
                collect_arguments_usage_from_expression(expression, indexed_slots, track_all_slots);
            }
        }
        Expression::Array(elements) => {
            for element in elements {
                match element {
                    crate::ir::hir::ArrayElement::Expression(expression)
                    | crate::ir::hir::ArrayElement::Spread(expression) => {
                        collect_arguments_usage_from_expression(
                            expression,
                            indexed_slots,
                            track_all_slots,
                        );
                    }
                }
            }
        }
        Expression::Object(entries) => {
            for entry in entries {
                match entry {
                    crate::ir::hir::ObjectEntry::Data { key, value } => {
                        collect_arguments_usage_from_expression(
                            key,
                            indexed_slots,
                            track_all_slots,
                        );
                        collect_arguments_usage_from_expression(
                            value,
                            indexed_slots,
                            track_all_slots,
                        );
                    }
                    crate::ir::hir::ObjectEntry::Getter { key, getter }
                    | crate::ir::hir::ObjectEntry::Setter {
                        key,
                        setter: getter,
                    } => {
                        collect_arguments_usage_from_expression(
                            key,
                            indexed_slots,
                            track_all_slots,
                        );
                        collect_arguments_usage_from_expression(
                            getter,
                            indexed_slots,
                            track_all_slots,
                        );
                    }
                    crate::ir::hir::ObjectEntry::Spread(value) => {
                        collect_arguments_usage_from_expression(
                            value,
                            indexed_slots,
                            track_all_slots,
                        );
                    }
                }
            }
        }
        Expression::Call { callee, arguments }
        | Expression::SuperCall { callee, arguments }
        | Expression::New { callee, arguments } => {
            collect_arguments_usage_from_expression(callee, indexed_slots, track_all_slots);
            for argument in arguments {
                match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                        collect_arguments_usage_from_expression(
                            expression,
                            indexed_slots,
                            track_all_slots,
                        );
                    }
                }
            }
        }
        Expression::SuperMember { property } => {
            collect_arguments_usage_from_expression(property, indexed_slots, track_all_slots);
        }
        Expression::AssignSuperMember { property, value } => {
            collect_arguments_usage_from_expression(property, indexed_slots, track_all_slots);
            collect_arguments_usage_from_expression(value, indexed_slots, track_all_slots);
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
