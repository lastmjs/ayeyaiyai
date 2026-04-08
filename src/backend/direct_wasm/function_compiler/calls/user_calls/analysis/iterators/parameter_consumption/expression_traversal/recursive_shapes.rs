use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn collect_parameter_get_iterator_names_from_children(
        expression: &Expression,
        param_names: &HashSet<String>,
        consumed_names: &mut HashSet<String>,
    ) {
        match expression {
            Expression::GetIterator(value)
            | Expression::Assign { value, .. }
            | Expression::Await(value)
            | Expression::EnumerateKeys(value)
            | Expression::IteratorClose(value)
            | Expression::Unary {
                expression: value, ..
            } => Self::collect_parameter_get_iterator_names_from_expression(
                value,
                param_names,
                consumed_names,
            ),
            Expression::Member { object, property } => {
                Self::collect_parameter_get_iterator_names_from_expression(
                    object,
                    param_names,
                    consumed_names,
                );
                Self::collect_parameter_get_iterator_names_from_expression(
                    property,
                    param_names,
                    consumed_names,
                );
            }
            Expression::SuperMember { property } => {
                Self::collect_parameter_get_iterator_names_from_expression(
                    property,
                    param_names,
                    consumed_names,
                );
            }
            Expression::AssignMember {
                object,
                property,
                value,
            } => {
                Self::collect_parameter_get_iterator_names_from_expression(
                    object,
                    param_names,
                    consumed_names,
                );
                Self::collect_parameter_get_iterator_names_from_expression(
                    property,
                    param_names,
                    consumed_names,
                );
                Self::collect_parameter_get_iterator_names_from_expression(
                    value,
                    param_names,
                    consumed_names,
                );
            }
            Expression::AssignSuperMember { property, value } => {
                Self::collect_parameter_get_iterator_names_from_expression(
                    property,
                    param_names,
                    consumed_names,
                );
                Self::collect_parameter_get_iterator_names_from_expression(
                    value,
                    param_names,
                    consumed_names,
                );
            }
            Expression::Binary { left, right, .. } => {
                Self::collect_parameter_get_iterator_names_from_expression(
                    left,
                    param_names,
                    consumed_names,
                );
                Self::collect_parameter_get_iterator_names_from_expression(
                    right,
                    param_names,
                    consumed_names,
                );
            }
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                Self::collect_parameter_get_iterator_names_from_expression(
                    condition,
                    param_names,
                    consumed_names,
                );
                Self::collect_parameter_get_iterator_names_from_expression(
                    then_expression,
                    param_names,
                    consumed_names,
                );
                Self::collect_parameter_get_iterator_names_from_expression(
                    else_expression,
                    param_names,
                    consumed_names,
                );
            }
            Expression::Sequence(expressions) => {
                for expression in expressions {
                    Self::collect_parameter_get_iterator_names_from_expression(
                        expression,
                        param_names,
                        consumed_names,
                    );
                }
            }
            Expression::Call { callee, arguments }
            | Expression::SuperCall { callee, arguments }
            | Expression::New { callee, arguments } => {
                Self::collect_parameter_get_iterator_names_from_expression(
                    callee,
                    param_names,
                    consumed_names,
                );
                for argument in arguments {
                    match argument {
                        CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                            Self::collect_parameter_get_iterator_names_from_expression(
                                expression,
                                param_names,
                                consumed_names,
                            );
                        }
                    }
                }
            }
            Expression::Array(elements) => {
                for element in elements {
                    match element {
                        ArrayElement::Expression(expression) | ArrayElement::Spread(expression) => {
                            Self::collect_parameter_get_iterator_names_from_expression(
                                expression,
                                param_names,
                                consumed_names,
                            );
                        }
                    }
                }
            }
            Expression::Object(entries) => {
                for entry in entries {
                    match entry {
                        ObjectEntry::Data { key, value } => {
                            Self::collect_parameter_get_iterator_names_from_expression(
                                key,
                                param_names,
                                consumed_names,
                            );
                            Self::collect_parameter_get_iterator_names_from_expression(
                                value,
                                param_names,
                                consumed_names,
                            );
                        }
                        ObjectEntry::Getter { key, getter } => {
                            Self::collect_parameter_get_iterator_names_from_expression(
                                key,
                                param_names,
                                consumed_names,
                            );
                            Self::collect_parameter_get_iterator_names_from_expression(
                                getter,
                                param_names,
                                consumed_names,
                            );
                        }
                        ObjectEntry::Setter { key, setter } => {
                            Self::collect_parameter_get_iterator_names_from_expression(
                                key,
                                param_names,
                                consumed_names,
                            );
                            Self::collect_parameter_get_iterator_names_from_expression(
                                setter,
                                param_names,
                                consumed_names,
                            );
                        }
                        ObjectEntry::Spread(expression) => {
                            Self::collect_parameter_get_iterator_names_from_expression(
                                expression,
                                param_names,
                                consumed_names,
                            );
                        }
                    }
                }
            }
            Expression::Identifier(_)
            | Expression::Number(_)
            | Expression::BigInt(_)
            | Expression::String(_)
            | Expression::Bool(_)
            | Expression::Null
            | Expression::Undefined
            | Expression::This
            | Expression::NewTarget
            | Expression::Sent
            | Expression::Update { .. } => {}
        }
    }
}
