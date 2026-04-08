use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_generator_call_this_binding(
        &self,
        callee: &Expression,
    ) -> Expression {
        match callee {
            Expression::Member { object, .. } => object.as_ref().clone(),
            _ => Expression::Undefined,
        }
    }

    pub(in crate::backend::direct_wasm) fn statement_contains_generator_yield(
        statement: &Statement,
    ) -> bool {
        match statement {
            Statement::Yield { .. } | Statement::YieldDelegate { .. } => true,
            Statement::Block { body } => body.iter().any(Self::statement_contains_generator_yield),
            Statement::If {
                then_branch,
                else_branch,
                ..
            } => {
                then_branch
                    .iter()
                    .any(Self::statement_contains_generator_yield)
                    || else_branch
                        .iter()
                        .any(Self::statement_contains_generator_yield)
            }
            _ => false,
        }
    }

    pub(in crate::backend::direct_wasm) fn substitute_sent_expression(
        expression: &Expression,
        replacement: &Expression,
    ) -> Expression {
        match expression {
            Expression::Sent => replacement.clone(),
            Expression::Member { object, property } => Expression::Member {
                object: Box::new(Self::substitute_sent_expression(object, replacement)),
                property: Box::new(Self::substitute_sent_expression(property, replacement)),
            },
            Expression::SuperMember { property } => Expression::SuperMember {
                property: Box::new(Self::substitute_sent_expression(property, replacement)),
            },
            Expression::Assign { name, value } => Expression::Assign {
                name: name.clone(),
                value: Box::new(Self::substitute_sent_expression(value, replacement)),
            },
            Expression::AssignMember {
                object,
                property,
                value,
            } => Expression::AssignMember {
                object: Box::new(Self::substitute_sent_expression(object, replacement)),
                property: Box::new(Self::substitute_sent_expression(property, replacement)),
                value: Box::new(Self::substitute_sent_expression(value, replacement)),
            },
            Expression::AssignSuperMember { property, value } => Expression::AssignSuperMember {
                property: Box::new(Self::substitute_sent_expression(property, replacement)),
                value: Box::new(Self::substitute_sent_expression(value, replacement)),
            },
            Expression::Await(value) => Expression::Await(Box::new(
                Self::substitute_sent_expression(value, replacement),
            )),
            Expression::EnumerateKeys(value) => Expression::EnumerateKeys(Box::new(
                Self::substitute_sent_expression(value, replacement),
            )),
            Expression::GetIterator(value) => Expression::GetIterator(Box::new(
                Self::substitute_sent_expression(value, replacement),
            )),
            Expression::IteratorClose(value) => Expression::IteratorClose(Box::new(
                Self::substitute_sent_expression(value, replacement),
            )),
            Expression::Unary { op, expression } => Expression::Unary {
                op: *op,
                expression: Box::new(Self::substitute_sent_expression(expression, replacement)),
            },
            Expression::Binary { op, left, right } => Expression::Binary {
                op: *op,
                left: Box::new(Self::substitute_sent_expression(left, replacement)),
                right: Box::new(Self::substitute_sent_expression(right, replacement)),
            },
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => Expression::Conditional {
                condition: Box::new(Self::substitute_sent_expression(condition, replacement)),
                then_expression: Box::new(Self::substitute_sent_expression(
                    then_expression,
                    replacement,
                )),
                else_expression: Box::new(Self::substitute_sent_expression(
                    else_expression,
                    replacement,
                )),
            },
            Expression::Sequence(expressions) => Expression::Sequence(
                expressions
                    .iter()
                    .map(|expression| Self::substitute_sent_expression(expression, replacement))
                    .collect(),
            ),
            Expression::Call { callee, arguments } => Expression::Call {
                callee: Box::new(Self::substitute_sent_expression(callee, replacement)),
                arguments: arguments
                    .iter()
                    .map(|argument| match argument {
                        CallArgument::Expression(expression) => CallArgument::Expression(
                            Self::substitute_sent_expression(expression, replacement),
                        ),
                        CallArgument::Spread(expression) => CallArgument::Spread(
                            Self::substitute_sent_expression(expression, replacement),
                        ),
                    })
                    .collect(),
            },
            Expression::SuperCall { callee, arguments } => Expression::SuperCall {
                callee: Box::new(Self::substitute_sent_expression(callee, replacement)),
                arguments: arguments
                    .iter()
                    .map(|argument| match argument {
                        CallArgument::Expression(expression) => CallArgument::Expression(
                            Self::substitute_sent_expression(expression, replacement),
                        ),
                        CallArgument::Spread(expression) => CallArgument::Spread(
                            Self::substitute_sent_expression(expression, replacement),
                        ),
                    })
                    .collect(),
            },
            Expression::New { callee, arguments } => Expression::New {
                callee: Box::new(Self::substitute_sent_expression(callee, replacement)),
                arguments: arguments
                    .iter()
                    .map(|argument| match argument {
                        CallArgument::Expression(expression) => CallArgument::Expression(
                            Self::substitute_sent_expression(expression, replacement),
                        ),
                        CallArgument::Spread(expression) => CallArgument::Spread(
                            Self::substitute_sent_expression(expression, replacement),
                        ),
                    })
                    .collect(),
            },
            Expression::Array(elements) => Expression::Array(
                elements
                    .iter()
                    .map(|element| match element {
                        ArrayElement::Expression(expression) => ArrayElement::Expression(
                            Self::substitute_sent_expression(expression, replacement),
                        ),
                        ArrayElement::Spread(expression) => ArrayElement::Spread(
                            Self::substitute_sent_expression(expression, replacement),
                        ),
                    })
                    .collect(),
            ),
            Expression::Object(entries) => Expression::Object(
                entries
                    .iter()
                    .map(|entry| match entry {
                        ObjectEntry::Data { key, value } => ObjectEntry::Data {
                            key: Self::substitute_sent_expression(key, replacement),
                            value: Self::substitute_sent_expression(value, replacement),
                        },
                        ObjectEntry::Getter { key, getter } => ObjectEntry::Getter {
                            key: Self::substitute_sent_expression(key, replacement),
                            getter: Self::substitute_sent_expression(getter, replacement),
                        },
                        ObjectEntry::Setter { key, setter } => ObjectEntry::Setter {
                            key: Self::substitute_sent_expression(key, replacement),
                            setter: Self::substitute_sent_expression(setter, replacement),
                        },
                        ObjectEntry::Spread(expression) => ObjectEntry::Spread(
                            Self::substitute_sent_expression(expression, replacement),
                        ),
                    })
                    .collect(),
            ),
            _ => expression.clone(),
        }
    }

    pub(in crate::backend::direct_wasm) fn substitute_sent_statement(
        statement: &Statement,
        replacement: &Expression,
    ) -> Statement {
        match statement {
            Statement::Block { body } => Statement::Block {
                body: body
                    .iter()
                    .map(|statement| Self::substitute_sent_statement(statement, replacement))
                    .collect(),
            },
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => Statement::If {
                condition: Self::substitute_sent_expression(condition, replacement),
                then_branch: then_branch
                    .iter()
                    .map(|statement| Self::substitute_sent_statement(statement, replacement))
                    .collect(),
                else_branch: else_branch
                    .iter()
                    .map(|statement| Self::substitute_sent_statement(statement, replacement))
                    .collect(),
            },
            Statement::Var { name, value } => Statement::Var {
                name: name.clone(),
                value: Self::substitute_sent_expression(value, replacement),
            },
            Statement::Let {
                name,
                mutable,
                value,
            } => Statement::Let {
                name: name.clone(),
                mutable: *mutable,
                value: Self::substitute_sent_expression(value, replacement),
            },
            Statement::Assign { name, value } => Statement::Assign {
                name: name.clone(),
                value: Self::substitute_sent_expression(value, replacement),
            },
            Statement::AssignMember {
                object,
                property,
                value,
            } => Statement::AssignMember {
                object: Self::substitute_sent_expression(object, replacement),
                property: Self::substitute_sent_expression(property, replacement),
                value: Self::substitute_sent_expression(value, replacement),
            },
            Statement::Expression(expression) => {
                Statement::Expression(Self::substitute_sent_expression(expression, replacement))
            }
            Statement::Print { values } => Statement::Print {
                values: values
                    .iter()
                    .map(|value| Self::substitute_sent_expression(value, replacement))
                    .collect(),
            },
            Statement::Throw(expression) => {
                Statement::Throw(Self::substitute_sent_expression(expression, replacement))
            }
            Statement::Return(expression) => {
                Statement::Return(Self::substitute_sent_expression(expression, replacement))
            }
            Statement::Yield { value } => Statement::Yield {
                value: Self::substitute_sent_expression(value, replacement),
            },
            Statement::YieldDelegate { value } => Statement::YieldDelegate {
                value: Self::substitute_sent_expression(value, replacement),
            },
            _ => statement.clone(),
        }
    }
}
