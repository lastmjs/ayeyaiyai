use super::*;

impl DirectWasmCompiler {
    pub(super) fn collect_static_direct_eval_assigned_nonlocal_names_from_expression_recursive(
        &self,
        expression: &Expression,
        current_function_name: Option<&str>,
        names: &mut HashSet<String>,
    ) {
        match expression {
            Expression::Member { object, property } => {
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    object,
                    current_function_name,
                    names,
                );
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    property,
                    current_function_name,
                    names,
                );
            }
            Expression::SuperMember { property }
            | Expression::Await(property)
            | Expression::EnumerateKeys(property)
            | Expression::GetIterator(property)
            | Expression::IteratorClose(property)
            | Expression::Unary {
                expression: property,
                ..
            }
            | Expression::Assign {
                value: property, ..
            } => {
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    property,
                    current_function_name,
                    names,
                );
            }
            Expression::AssignMember {
                object,
                property,
                value,
            } => {
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    object,
                    current_function_name,
                    names,
                );
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    property,
                    current_function_name,
                    names,
                );
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    value,
                    current_function_name,
                    names,
                );
            }
            Expression::AssignSuperMember { property, value } => {
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    property,
                    current_function_name,
                    names,
                );
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    value,
                    current_function_name,
                    names,
                );
            }
            Expression::Binary { left, right, .. } => {
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    left,
                    current_function_name,
                    names,
                );
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    right,
                    current_function_name,
                    names,
                );
            }
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    condition,
                    current_function_name,
                    names,
                );
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    then_expression,
                    current_function_name,
                    names,
                );
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    else_expression,
                    current_function_name,
                    names,
                );
            }
            Expression::Sequence(expressions) => {
                for expression in expressions {
                    self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                        expression,
                        current_function_name,
                        names,
                    );
                }
            }
            Expression::SuperCall { callee, arguments }
            | Expression::New { callee, arguments }
            | Expression::Call { callee, arguments } => {
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    callee,
                    current_function_name,
                    names,
                );
                self.collect_static_direct_eval_assigned_nonlocal_names_from_call_arguments(
                    arguments,
                    current_function_name,
                    names,
                );
            }
            Expression::Array(elements) => {
                for element in elements {
                    match element {
                        ArrayElement::Expression(expression) | ArrayElement::Spread(expression) => {
                            self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                                expression,
                                current_function_name,
                                names,
                            );
                        }
                    }
                }
            }
            Expression::Object(entries) => {
                for entry in entries {
                    match entry {
                        ObjectEntry::Data { key, value } => {
                            self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                                key,
                                current_function_name,
                                names,
                            );
                            self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                                value,
                                current_function_name,
                                names,
                            );
                        }
                        ObjectEntry::Getter { key, getter } => {
                            self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                                key,
                                current_function_name,
                                names,
                            );
                            self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                                getter,
                                current_function_name,
                                names,
                            );
                        }
                        ObjectEntry::Setter { key, setter } => {
                            self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                                key,
                                current_function_name,
                                names,
                            );
                            self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                                setter,
                                current_function_name,
                                names,
                            );
                        }
                        ObjectEntry::Spread(expression) => {
                            self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                                expression,
                                current_function_name,
                                names,
                            );
                        }
                    }
                }
            }
            Expression::Identifier(_)
            | Expression::Update { .. }
            | Expression::Number(_)
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

    pub(super) fn collect_static_direct_eval_assigned_nonlocal_names_from_call_arguments(
        &self,
        arguments: &[CallArgument],
        current_function_name: Option<&str>,
        names: &mut HashSet<String>,
    ) {
        for argument in arguments {
            match argument {
                CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                    self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                        expression,
                        current_function_name,
                        names,
                    );
                }
            }
        }
    }
}
