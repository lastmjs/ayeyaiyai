use super::*;

pub(in crate::backend::direct_wasm) fn collect_returned_member_function_bindings(
    statements: &[Statement],
    function_names: &HashSet<String>,
) -> Vec<ReturnedMemberFunctionBinding> {
    let Some(returned_identifier) = collect_returned_identifier(statements) else {
        return Vec::new();
    };

    let mut bindings = HashMap::new();
    for statement in statements {
        collect_returned_member_function_bindings_from_statement(
            statement,
            &returned_identifier,
            function_names,
            &mut bindings,
        );
    }

    bindings
        .into_iter()
        .map(|(key, binding)| ReturnedMemberFunctionBinding {
            target: key.target,
            property: key.property,
            binding,
        })
        .collect()
}

pub(in crate::backend::direct_wasm) fn collect_returned_member_function_bindings_from_statement(
    statement: &Statement,
    returned_identifier: &str,
    function_names: &HashSet<String>,
    bindings: &mut HashMap<ReturnedMemberFunctionBindingKey, LocalFunctionBinding>,
) {
    match statement {
        Statement::Block { body }
        | Statement::Labeled { body, .. }
        | Statement::With { body, .. } => {
            for statement in body {
                collect_returned_member_function_bindings_from_statement(
                    statement,
                    returned_identifier,
                    function_names,
                    bindings,
                );
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
            collect_returned_member_function_bindings_from_expression(
                value,
                returned_identifier,
                function_names,
                bindings,
            );
        }
        Statement::AssignMember {
            object,
            property,
            value,
        } => {
            collect_returned_member_function_bindings_from_expression(
                object,
                returned_identifier,
                function_names,
                bindings,
            );
            collect_returned_member_function_bindings_from_expression(
                property,
                returned_identifier,
                function_names,
                bindings,
            );
            collect_returned_member_function_bindings_from_expression(
                value,
                returned_identifier,
                function_names,
                bindings,
            );
        }
        Statement::Print { values } => {
            for value in values {
                collect_returned_member_function_bindings_from_expression(
                    value,
                    returned_identifier,
                    function_names,
                    bindings,
                );
            }
        }
        Statement::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_returned_member_function_bindings_from_expression(
                condition,
                returned_identifier,
                function_names,
                bindings,
            );
            for statement in then_branch {
                collect_returned_member_function_bindings_from_statement(
                    statement,
                    returned_identifier,
                    function_names,
                    bindings,
                );
            }
            for statement in else_branch {
                collect_returned_member_function_bindings_from_statement(
                    statement,
                    returned_identifier,
                    function_names,
                    bindings,
                );
            }
        }
        Statement::Try {
            body,
            catch_setup,
            catch_body,
            ..
        } => {
            for statement in body {
                collect_returned_member_function_bindings_from_statement(
                    statement,
                    returned_identifier,
                    function_names,
                    bindings,
                );
            }
            for statement in catch_setup {
                collect_returned_member_function_bindings_from_statement(
                    statement,
                    returned_identifier,
                    function_names,
                    bindings,
                );
            }
            for statement in catch_body {
                collect_returned_member_function_bindings_from_statement(
                    statement,
                    returned_identifier,
                    function_names,
                    bindings,
                );
            }
        }
        Statement::Switch {
            discriminant,
            cases,
            ..
        } => {
            collect_returned_member_function_bindings_from_expression(
                discriminant,
                returned_identifier,
                function_names,
                bindings,
            );
            for case in cases {
                if let Some(test) = &case.test {
                    collect_returned_member_function_bindings_from_expression(
                        test,
                        returned_identifier,
                        function_names,
                        bindings,
                    );
                }
                for statement in &case.body {
                    collect_returned_member_function_bindings_from_statement(
                        statement,
                        returned_identifier,
                        function_names,
                        bindings,
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
                collect_returned_member_function_bindings_from_statement(
                    statement,
                    returned_identifier,
                    function_names,
                    bindings,
                );
            }
            if let Some(condition) = condition {
                collect_returned_member_function_bindings_from_expression(
                    condition,
                    returned_identifier,
                    function_names,
                    bindings,
                );
            }
            if let Some(update) = update {
                collect_returned_member_function_bindings_from_expression(
                    update,
                    returned_identifier,
                    function_names,
                    bindings,
                );
            }
            if let Some(break_hook) = break_hook {
                collect_returned_member_function_bindings_from_expression(
                    break_hook,
                    returned_identifier,
                    function_names,
                    bindings,
                );
            }
            for statement in body {
                collect_returned_member_function_bindings_from_statement(
                    statement,
                    returned_identifier,
                    function_names,
                    bindings,
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
            collect_returned_member_function_bindings_from_expression(
                condition,
                returned_identifier,
                function_names,
                bindings,
            );
            if let Some(break_hook) = break_hook {
                collect_returned_member_function_bindings_from_expression(
                    break_hook,
                    returned_identifier,
                    function_names,
                    bindings,
                );
            }
            for statement in body {
                collect_returned_member_function_bindings_from_statement(
                    statement,
                    returned_identifier,
                    function_names,
                    bindings,
                );
            }
        }
        _ => {}
    }
}

pub(in crate::backend::direct_wasm) fn collect_returned_member_function_bindings_from_expression(
    expression: &Expression,
    returned_identifier: &str,
    function_names: &HashSet<String>,
    bindings: &mut HashMap<ReturnedMemberFunctionBindingKey, LocalFunctionBinding>,
) {
    match expression {
        Expression::Call { callee, arguments }
        | Expression::SuperCall { callee, arguments }
        | Expression::New { callee, arguments } => {
            collect_returned_member_function_bindings_from_expression(
                callee,
                returned_identifier,
                function_names,
                bindings,
            );
            for argument in arguments {
                match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                        collect_returned_member_function_bindings_from_expression(
                            expression,
                            returned_identifier,
                            function_names,
                            bindings,
                        );
                    }
                }
            }

            let Expression::Member { object, property } = callee.as_ref() else {
                return;
            };
            if !matches!(object.as_ref(), Expression::Identifier(name) if name == "Object") {
                return;
            }
            if !matches!(property.as_ref(), Expression::String(name) if name == "defineProperty") {
                return;
            }
            let [
                CallArgument::Expression(target),
                CallArgument::Expression(property),
                CallArgument::Expression(descriptor),
                ..,
            ] = arguments.as_slice()
            else {
                return;
            };
            let Some(key) =
                returned_member_function_binding_key(target, property, returned_identifier)
            else {
                return;
            };
            let Some(binding) = resolve_returned_member_function_binding_from_descriptor(
                descriptor,
                returned_identifier,
                function_names,
                bindings,
            ) else {
                bindings.remove(&key);
                return;
            };
            bindings.insert(key, binding);
        }
        Expression::AssignMember {
            object,
            property,
            value,
        } => {
            collect_returned_member_function_bindings_from_expression(
                object,
                returned_identifier,
                function_names,
                bindings,
            );
            collect_returned_member_function_bindings_from_expression(
                property,
                returned_identifier,
                function_names,
                bindings,
            );
            collect_returned_member_function_bindings_from_expression(
                value,
                returned_identifier,
                function_names,
                bindings,
            );
        }
        Expression::Unary { expression, .. }
        | Expression::Await(expression)
        | Expression::EnumerateKeys(expression)
        | Expression::GetIterator(expression)
        | Expression::IteratorClose(expression) => {
            collect_returned_member_function_bindings_from_expression(
                expression,
                returned_identifier,
                function_names,
                bindings,
            );
        }
        Expression::Binary { left, right, .. } => {
            collect_returned_member_function_bindings_from_expression(
                left,
                returned_identifier,
                function_names,
                bindings,
            );
            collect_returned_member_function_bindings_from_expression(
                right,
                returned_identifier,
                function_names,
                bindings,
            );
        }
        Expression::Conditional {
            condition,
            then_expression,
            else_expression,
        } => {
            collect_returned_member_function_bindings_from_expression(
                condition,
                returned_identifier,
                function_names,
                bindings,
            );
            collect_returned_member_function_bindings_from_expression(
                then_expression,
                returned_identifier,
                function_names,
                bindings,
            );
            collect_returned_member_function_bindings_from_expression(
                else_expression,
                returned_identifier,
                function_names,
                bindings,
            );
        }
        Expression::Sequence(expressions) => {
            for expression in expressions {
                collect_returned_member_function_bindings_from_expression(
                    expression,
                    returned_identifier,
                    function_names,
                    bindings,
                );
            }
        }
        Expression::Member { object, property } => {
            collect_returned_member_function_bindings_from_expression(
                object,
                returned_identifier,
                function_names,
                bindings,
            );
            collect_returned_member_function_bindings_from_expression(
                property,
                returned_identifier,
                function_names,
                bindings,
            );
        }
        Expression::Assign { value, .. } | Expression::AssignSuperMember { value, .. } => {
            collect_returned_member_function_bindings_from_expression(
                value,
                returned_identifier,
                function_names,
                bindings,
            );
        }
        Expression::SuperMember { property } => {
            collect_returned_member_function_bindings_from_expression(
                property,
                returned_identifier,
                function_names,
                bindings,
            );
        }
        Expression::Array(elements) => {
            for element in elements {
                match element {
                    crate::ir::hir::ArrayElement::Expression(expression)
                    | crate::ir::hir::ArrayElement::Spread(expression) => {
                        collect_returned_member_function_bindings_from_expression(
                            expression,
                            returned_identifier,
                            function_names,
                            bindings,
                        );
                    }
                }
            }
        }
        Expression::Object(entries) => {
            for entry in entries {
                match entry {
                    crate::ir::hir::ObjectEntry::Data { key, value } => {
                        collect_returned_member_function_bindings_from_expression(
                            key,
                            returned_identifier,
                            function_names,
                            bindings,
                        );
                        collect_returned_member_function_bindings_from_expression(
                            value,
                            returned_identifier,
                            function_names,
                            bindings,
                        );
                    }
                    crate::ir::hir::ObjectEntry::Getter { key, getter }
                    | crate::ir::hir::ObjectEntry::Setter {
                        key,
                        setter: getter,
                    } => {
                        collect_returned_member_function_bindings_from_expression(
                            key,
                            returned_identifier,
                            function_names,
                            bindings,
                        );
                        collect_returned_member_function_bindings_from_expression(
                            getter,
                            returned_identifier,
                            function_names,
                            bindings,
                        );
                    }
                    crate::ir::hir::ObjectEntry::Spread(value) => {
                        collect_returned_member_function_bindings_from_expression(
                            value,
                            returned_identifier,
                            function_names,
                            bindings,
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
        | Expression::Identifier(_)
        | Expression::This
        | Expression::Sent
        | Expression::Update { .. } => {}
    }
}

pub(in crate::backend::direct_wasm) fn returned_member_function_binding_key(
    object: &Expression,
    property: &Expression,
    returned_identifier: &str,
) -> Option<ReturnedMemberFunctionBindingKey> {
    let Expression::String(property_name) = property else {
        return None;
    };

    let target = match object {
        Expression::Identifier(name) if name == returned_identifier => {
            ReturnedMemberFunctionBindingTarget::Value
        }
        Expression::Member { object, property }
            if matches!(property.as_ref(), Expression::String(name) if name == "prototype")
                && matches!(object.as_ref(), Expression::Identifier(name) if name == returned_identifier) =>
        {
            ReturnedMemberFunctionBindingTarget::Prototype
        }
        _ => return None,
    };

    Some(ReturnedMemberFunctionBindingKey {
        target,
        property: property_name.clone(),
    })
}

pub(in crate::backend::direct_wasm) fn resolve_returned_member_function_binding_from_descriptor(
    descriptor: &Expression,
    returned_identifier: &str,
    function_names: &HashSet<String>,
    bindings: &HashMap<ReturnedMemberFunctionBindingKey, LocalFunctionBinding>,
) -> Option<LocalFunctionBinding> {
    let Expression::Object(entries) = descriptor else {
        return None;
    };
    for entry in entries {
        let crate::ir::hir::ObjectEntry::Data { key, value } = entry else {
            continue;
        };
        if matches!(key, Expression::String(name) if name == "value") {
            return resolve_returned_member_function_binding(
                value,
                returned_identifier,
                function_names,
                bindings,
            );
        }
    }
    None
}

pub(in crate::backend::direct_wasm) fn resolve_returned_member_function_binding(
    expression: &Expression,
    returned_identifier: &str,
    function_names: &HashSet<String>,
    bindings: &HashMap<ReturnedMemberFunctionBindingKey, LocalFunctionBinding>,
) -> Option<LocalFunctionBinding> {
    match expression {
        Expression::Identifier(name) if function_names.contains(name) => {
            Some(LocalFunctionBinding::User(name.clone()))
        }
        Expression::Member { object, property } => {
            let key = returned_member_function_binding_key(object, property, returned_identifier)?;
            bindings.get(&key).cloned()
        }
        _ => None,
    }
}
