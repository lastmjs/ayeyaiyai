use super::*;

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
