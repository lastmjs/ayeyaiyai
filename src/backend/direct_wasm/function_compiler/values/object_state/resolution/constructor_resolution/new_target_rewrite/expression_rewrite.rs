use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn substitute_static_constructor_new_target_expression(
        expression: &Expression,
    ) -> Expression {
        match expression {
            Expression::Member { object, property } => Expression::Member {
                object: Box::new(Self::substitute_static_constructor_new_target_expression(
                    object,
                )),
                property: Box::new(Self::substitute_static_constructor_new_target_expression(
                    property,
                )),
            },
            Expression::SuperMember { property } => Expression::SuperMember {
                property: Box::new(Self::substitute_static_constructor_new_target_expression(
                    property,
                )),
            },
            Expression::Assign { name, value } => Expression::Assign {
                name: name.clone(),
                value: Box::new(Self::substitute_static_constructor_new_target_expression(
                    value,
                )),
            },
            Expression::AssignMember {
                object,
                property,
                value,
            } => Expression::AssignMember {
                object: Box::new(Self::substitute_static_constructor_new_target_expression(
                    object,
                )),
                property: Box::new(Self::substitute_static_constructor_new_target_expression(
                    property,
                )),
                value: Box::new(Self::substitute_static_constructor_new_target_expression(
                    value,
                )),
            },
            Expression::AssignSuperMember { property, value } => Expression::AssignSuperMember {
                property: Box::new(Self::substitute_static_constructor_new_target_expression(
                    property,
                )),
                value: Box::new(Self::substitute_static_constructor_new_target_expression(
                    value,
                )),
            },
            Expression::Await(value) => Expression::Await(Box::new(
                Self::substitute_static_constructor_new_target_expression(value),
            )),
            Expression::EnumerateKeys(value) => Expression::EnumerateKeys(Box::new(
                Self::substitute_static_constructor_new_target_expression(value),
            )),
            Expression::GetIterator(value) => Expression::GetIterator(Box::new(
                Self::substitute_static_constructor_new_target_expression(value),
            )),
            Expression::IteratorClose(value) => Expression::IteratorClose(Box::new(
                Self::substitute_static_constructor_new_target_expression(value),
            )),
            Expression::Unary { op, expression } => Expression::Unary {
                op: *op,
                expression: Box::new(Self::substitute_static_constructor_new_target_expression(
                    expression,
                )),
            },
            Expression::Binary { op, left, right } => Expression::Binary {
                op: *op,
                left: Box::new(Self::substitute_static_constructor_new_target_expression(
                    left,
                )),
                right: Box::new(Self::substitute_static_constructor_new_target_expression(
                    right,
                )),
            },
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => Expression::Conditional {
                condition: Box::new(Self::substitute_static_constructor_new_target_expression(
                    condition,
                )),
                then_expression: Box::new(
                    Self::substitute_static_constructor_new_target_expression(then_expression),
                ),
                else_expression: Box::new(
                    Self::substitute_static_constructor_new_target_expression(else_expression),
                ),
            },
            Expression::Sequence(expressions) => Expression::Sequence(
                expressions
                    .iter()
                    .map(Self::substitute_static_constructor_new_target_expression)
                    .collect(),
            ),
            Expression::Call { callee, arguments } => Expression::Call {
                callee: Box::new(Self::substitute_static_constructor_new_target_expression(
                    callee,
                )),
                arguments: arguments
                    .iter()
                    .map(|argument| match argument {
                        CallArgument::Expression(expression) => CallArgument::Expression(
                            Self::substitute_static_constructor_new_target_expression(expression),
                        ),
                        CallArgument::Spread(expression) => CallArgument::Spread(
                            Self::substitute_static_constructor_new_target_expression(expression),
                        ),
                    })
                    .collect(),
            },
            Expression::SuperCall { callee, arguments } => Expression::SuperCall {
                callee: Box::new(Self::substitute_static_constructor_new_target_expression(
                    callee,
                )),
                arguments: arguments
                    .iter()
                    .map(|argument| match argument {
                        CallArgument::Expression(expression) => CallArgument::Expression(
                            Self::substitute_static_constructor_new_target_expression(expression),
                        ),
                        CallArgument::Spread(expression) => CallArgument::Spread(
                            Self::substitute_static_constructor_new_target_expression(expression),
                        ),
                    })
                    .collect(),
            },
            Expression::New { callee, arguments } => Expression::New {
                callee: Box::new(Self::substitute_static_constructor_new_target_expression(
                    callee,
                )),
                arguments: arguments
                    .iter()
                    .map(|argument| match argument {
                        CallArgument::Expression(expression) => CallArgument::Expression(
                            Self::substitute_static_constructor_new_target_expression(expression),
                        ),
                        CallArgument::Spread(expression) => CallArgument::Spread(
                            Self::substitute_static_constructor_new_target_expression(expression),
                        ),
                    })
                    .collect(),
            },
            Expression::Array(elements) => Expression::Array(
                elements
                    .iter()
                    .map(|element| match element {
                        ArrayElement::Expression(expression) => ArrayElement::Expression(
                            Self::substitute_static_constructor_new_target_expression(expression),
                        ),
                        ArrayElement::Spread(expression) => ArrayElement::Spread(
                            Self::substitute_static_constructor_new_target_expression(expression),
                        ),
                    })
                    .collect(),
            ),
            Expression::Object(entries) => Expression::Object(
                entries
                    .iter()
                    .map(|entry| match entry {
                        ObjectEntry::Data { key, value } => ObjectEntry::Data {
                            key: Self::substitute_static_constructor_new_target_expression(key),
                            value: Self::substitute_static_constructor_new_target_expression(value),
                        },
                        ObjectEntry::Getter { key, getter } => ObjectEntry::Getter {
                            key: Self::substitute_static_constructor_new_target_expression(key),
                            getter: Self::substitute_static_constructor_new_target_expression(
                                getter,
                            ),
                        },
                        ObjectEntry::Setter { key, setter } => ObjectEntry::Setter {
                            key: Self::substitute_static_constructor_new_target_expression(key),
                            setter: Self::substitute_static_constructor_new_target_expression(
                                setter,
                            ),
                        },
                        ObjectEntry::Spread(expression) => ObjectEntry::Spread(
                            Self::substitute_static_constructor_new_target_expression(expression),
                        ),
                    })
                    .collect(),
            ),
            Expression::NewTarget => Expression::Bool(true),
            _ => expression.clone(),
        }
    }
}
