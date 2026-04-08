use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn rewrite_eval_scoped_captures_in_expression(
        expression: &mut Expression,
        declared_bindings: &HashSet<String>,
        eval_local_function_bindings: &HashSet<String>,
    ) {
        match expression {
            Expression::Identifier(name) | Expression::Update { name, .. } => {
                Self::rewrite_eval_scoped_binding_name(
                    name,
                    declared_bindings,
                    eval_local_function_bindings,
                );
            }
            Expression::Array(elements) => {
                for element in elements {
                    match element {
                        crate::ir::hir::ArrayElement::Expression(expression)
                        | crate::ir::hir::ArrayElement::Spread(expression) => {
                            Self::rewrite_eval_scoped_captures_in_expression(
                                expression,
                                declared_bindings,
                                eval_local_function_bindings,
                            );
                        }
                    }
                }
            }
            Expression::Object(entries) => {
                for entry in entries {
                    match entry {
                        ObjectEntry::Data { key, value } => {
                            Self::rewrite_eval_scoped_captures_in_expression(
                                key,
                                declared_bindings,
                                eval_local_function_bindings,
                            );
                            Self::rewrite_eval_scoped_captures_in_expression(
                                value,
                                declared_bindings,
                                eval_local_function_bindings,
                            );
                        }
                        ObjectEntry::Getter { key, getter } => {
                            Self::rewrite_eval_scoped_captures_in_expression(
                                key,
                                declared_bindings,
                                eval_local_function_bindings,
                            );
                            Self::rewrite_eval_scoped_captures_in_expression(
                                getter,
                                declared_bindings,
                                eval_local_function_bindings,
                            );
                        }
                        ObjectEntry::Setter { key, setter } => {
                            Self::rewrite_eval_scoped_captures_in_expression(
                                key,
                                declared_bindings,
                                eval_local_function_bindings,
                            );
                            Self::rewrite_eval_scoped_captures_in_expression(
                                setter,
                                declared_bindings,
                                eval_local_function_bindings,
                            );
                        }
                        ObjectEntry::Spread(expression) => {
                            Self::rewrite_eval_scoped_captures_in_expression(
                                expression,
                                declared_bindings,
                                eval_local_function_bindings,
                            );
                        }
                    }
                }
            }
            Expression::Member { object, property } => {
                Self::rewrite_eval_scoped_captures_in_expression(
                    object,
                    declared_bindings,
                    eval_local_function_bindings,
                );
                Self::rewrite_eval_scoped_captures_in_expression(
                    property,
                    declared_bindings,
                    eval_local_function_bindings,
                );
            }
            Expression::SuperMember { property } => {
                Self::rewrite_eval_scoped_captures_in_expression(
                    property,
                    declared_bindings,
                    eval_local_function_bindings,
                );
            }
            Expression::Assign { name, value } => {
                Self::rewrite_eval_scoped_binding_name(
                    name,
                    declared_bindings,
                    eval_local_function_bindings,
                );
                Self::rewrite_eval_scoped_captures_in_expression(
                    value,
                    declared_bindings,
                    eval_local_function_bindings,
                );
            }
            Expression::AssignMember {
                object,
                property,
                value,
            } => {
                Self::rewrite_eval_scoped_captures_in_expression(
                    object,
                    declared_bindings,
                    eval_local_function_bindings,
                );
                Self::rewrite_eval_scoped_captures_in_expression(
                    property,
                    declared_bindings,
                    eval_local_function_bindings,
                );
                Self::rewrite_eval_scoped_captures_in_expression(
                    value,
                    declared_bindings,
                    eval_local_function_bindings,
                );
            }
            Expression::AssignSuperMember { property, value } => {
                Self::rewrite_eval_scoped_captures_in_expression(
                    property,
                    declared_bindings,
                    eval_local_function_bindings,
                );
                Self::rewrite_eval_scoped_captures_in_expression(
                    value,
                    declared_bindings,
                    eval_local_function_bindings,
                );
            }
            Expression::Await(expression)
            | Expression::EnumerateKeys(expression)
            | Expression::GetIterator(expression)
            | Expression::IteratorClose(expression)
            | Expression::Unary { expression, .. } => {
                Self::rewrite_eval_scoped_captures_in_expression(
                    expression,
                    declared_bindings,
                    eval_local_function_bindings,
                );
            }
            Expression::Binary { left, right, .. } => {
                Self::rewrite_eval_scoped_captures_in_expression(
                    left,
                    declared_bindings,
                    eval_local_function_bindings,
                );
                Self::rewrite_eval_scoped_captures_in_expression(
                    right,
                    declared_bindings,
                    eval_local_function_bindings,
                );
            }
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                Self::rewrite_eval_scoped_captures_in_expression(
                    condition,
                    declared_bindings,
                    eval_local_function_bindings,
                );
                Self::rewrite_eval_scoped_captures_in_expression(
                    then_expression,
                    declared_bindings,
                    eval_local_function_bindings,
                );
                Self::rewrite_eval_scoped_captures_in_expression(
                    else_expression,
                    declared_bindings,
                    eval_local_function_bindings,
                );
            }
            Expression::Sequence(expressions) => {
                for expression in expressions {
                    Self::rewrite_eval_scoped_captures_in_expression(
                        expression,
                        declared_bindings,
                        eval_local_function_bindings,
                    );
                }
            }
            Expression::Call { callee, arguments }
            | Expression::SuperCall { callee, arguments }
            | Expression::New { callee, arguments } => {
                Self::rewrite_eval_scoped_captures_in_expression(
                    callee,
                    declared_bindings,
                    eval_local_function_bindings,
                );
                for argument in arguments {
                    match argument {
                        CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                            Self::rewrite_eval_scoped_captures_in_expression(
                                expression,
                                declared_bindings,
                                eval_local_function_bindings,
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
}
