use super::*;

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
