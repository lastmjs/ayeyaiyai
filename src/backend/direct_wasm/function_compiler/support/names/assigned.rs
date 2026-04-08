use super::*;

pub(in crate::backend::direct_wasm) fn collect_assigned_binding_names_from_statement(
    statement: &Statement,
    names: &mut HashSet<String>,
) {
    match statement {
        Statement::Declaration { body }
        | Statement::Block { body }
        | Statement::Labeled { body, .. } => {
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
