use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn expression_contains_iterator_protocol_ops(
        expression: &Expression,
    ) -> bool {
        match expression {
            Expression::GetIterator(value) | Expression::IteratorClose(value) => {
                Self::expression_contains_iterator_protocol_ops(value) || true
            }
            Expression::Call { callee, arguments } => {
                if let Expression::Member { property, .. } = callee.as_ref()
                    && matches!(property.as_ref(), Expression::String(name) if matches!(name.as_str(), "next" | "return" | "throw"))
                {
                    return true;
                }
                Self::expression_contains_iterator_protocol_ops(callee)
                    || arguments.iter().any(|argument| match argument {
                        CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                            Self::expression_contains_iterator_protocol_ops(expression)
                        }
                    })
            }
            Expression::SuperCall { callee, arguments } | Expression::New { callee, arguments } => {
                Self::expression_contains_iterator_protocol_ops(callee)
                    || arguments.iter().any(|argument| match argument {
                        CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                            Self::expression_contains_iterator_protocol_ops(expression)
                        }
                    })
            }
            Expression::Member { object, property } => {
                Self::expression_contains_iterator_protocol_ops(object)
                    || Self::expression_contains_iterator_protocol_ops(property)
            }
            Expression::SuperMember { property } => {
                Self::expression_contains_iterator_protocol_ops(property)
            }
            Expression::Assign { value, .. }
            | Expression::Await(value)
            | Expression::EnumerateKeys(value)
            | Expression::Unary {
                expression: value, ..
            } => Self::expression_contains_iterator_protocol_ops(value),
            Expression::AssignMember {
                object,
                property,
                value,
            } => {
                Self::expression_contains_iterator_protocol_ops(object)
                    || Self::expression_contains_iterator_protocol_ops(property)
                    || Self::expression_contains_iterator_protocol_ops(value)
            }
            Expression::AssignSuperMember { property, value } => {
                Self::expression_contains_iterator_protocol_ops(property)
                    || Self::expression_contains_iterator_protocol_ops(value)
            }
            Expression::Binary { left, right, .. } => {
                Self::expression_contains_iterator_protocol_ops(left)
                    || Self::expression_contains_iterator_protocol_ops(right)
            }
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                Self::expression_contains_iterator_protocol_ops(condition)
                    || Self::expression_contains_iterator_protocol_ops(then_expression)
                    || Self::expression_contains_iterator_protocol_ops(else_expression)
            }
            Expression::Sequence(expressions) => expressions
                .iter()
                .any(Self::expression_contains_iterator_protocol_ops),
            Expression::Array(elements) => elements.iter().any(|element| match element {
                ArrayElement::Expression(expression) | ArrayElement::Spread(expression) => {
                    Self::expression_contains_iterator_protocol_ops(expression)
                }
            }),
            Expression::Object(entries) => entries.iter().any(|entry| match entry {
                ObjectEntry::Data { key, value } => {
                    Self::expression_contains_iterator_protocol_ops(key)
                        || Self::expression_contains_iterator_protocol_ops(value)
                }
                ObjectEntry::Getter { key, getter }
                | ObjectEntry::Setter {
                    key,
                    setter: getter,
                } => {
                    Self::expression_contains_iterator_protocol_ops(key)
                        || Self::expression_contains_iterator_protocol_ops(getter)
                }
                ObjectEntry::Spread(expression) => {
                    Self::expression_contains_iterator_protocol_ops(expression)
                }
            }),
            Expression::Update { .. }
            | Expression::Identifier(_)
            | Expression::This
            | Expression::Sent
            | Expression::NewTarget
            | Expression::Undefined
            | Expression::Null
            | Expression::Bool(_)
            | Expression::Number(_)
            | Expression::String(_)
            | Expression::BigInt(_) => false,
        }
    }

    pub(in crate::backend::direct_wasm) fn statement_contains_iterator_protocol_ops(
        statement: &Statement,
    ) -> bool {
        match statement {
            Statement::Var { value, .. }
            | Statement::Let { value, .. }
            | Statement::Assign { value, .. }
            | Statement::Expression(value)
            | Statement::Throw(value)
            | Statement::Return(value) => Self::expression_contains_iterator_protocol_ops(value),
            Statement::AssignMember {
                object,
                property,
                value,
            } => {
                Self::expression_contains_iterator_protocol_ops(object)
                    || Self::expression_contains_iterator_protocol_ops(property)
                    || Self::expression_contains_iterator_protocol_ops(value)
            }
            Statement::Print { values } => values
                .iter()
                .any(Self::expression_contains_iterator_protocol_ops),
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => {
                Self::expression_contains_iterator_protocol_ops(condition)
                    || then_branch
                        .iter()
                        .any(Self::statement_contains_iterator_protocol_ops)
                    || else_branch
                        .iter()
                        .any(Self::statement_contains_iterator_protocol_ops)
            }
            Statement::Block { body } => body
                .iter()
                .any(Self::statement_contains_iterator_protocol_ops),
            _ => false,
        }
    }
}
