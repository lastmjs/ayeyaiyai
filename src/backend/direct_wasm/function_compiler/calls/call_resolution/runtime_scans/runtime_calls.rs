use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn expression_contains_runtime_call(
        expression: &Expression,
    ) -> bool {
        match expression {
            Expression::Call { .. } | Expression::SuperCall { .. } | Expression::New { .. } => true,
            Expression::Member { object, property } => {
                Self::expression_contains_runtime_call(object)
                    || Self::expression_contains_runtime_call(property)
            }
            Expression::SuperMember { property } => {
                Self::expression_contains_runtime_call(property)
            }
            Expression::Assign { value, .. }
            | Expression::Await(value)
            | Expression::EnumerateKeys(value)
            | Expression::GetIterator(value)
            | Expression::IteratorClose(value)
            | Expression::Unary {
                expression: value, ..
            } => Self::expression_contains_runtime_call(value),
            Expression::AssignMember {
                object,
                property,
                value,
            } => {
                Self::expression_contains_runtime_call(object)
                    || Self::expression_contains_runtime_call(property)
                    || Self::expression_contains_runtime_call(value)
            }
            Expression::AssignSuperMember { property, value } => {
                Self::expression_contains_runtime_call(property)
                    || Self::expression_contains_runtime_call(value)
            }
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
            Expression::Binary { left, right, .. } => {
                Self::expression_contains_runtime_call(left)
                    || Self::expression_contains_runtime_call(right)
            }
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                Self::expression_contains_runtime_call(condition)
                    || Self::expression_contains_runtime_call(then_expression)
                    || Self::expression_contains_runtime_call(else_expression)
            }
            Expression::Sequence(expressions) => expressions
                .iter()
                .any(Self::expression_contains_runtime_call),
            Expression::Array(elements) => elements.iter().any(|element| match element {
                ArrayElement::Expression(expression) | ArrayElement::Spread(expression) => {
                    Self::expression_contains_runtime_call(expression)
                }
            }),
            Expression::Object(entries) => entries.iter().any(|entry| match entry {
                ObjectEntry::Data { key, value } => {
                    Self::expression_contains_runtime_call(key)
                        || Self::expression_contains_runtime_call(value)
                }
                ObjectEntry::Getter { key, getter }
                | ObjectEntry::Setter {
                    key,
                    setter: getter,
                } => {
                    Self::expression_contains_runtime_call(key)
                        || Self::expression_contains_runtime_call(getter)
                }
                ObjectEntry::Spread(expression) => {
                    Self::expression_contains_runtime_call(expression)
                }
            }),
        }
    }

    pub(in crate::backend::direct_wasm) fn statement_contains_runtime_call(
        statement: &Statement,
    ) -> bool {
        match statement {
            Statement::Var { value, .. }
            | Statement::Let { value, .. }
            | Statement::Assign { value, .. }
            | Statement::Expression(value)
            | Statement::Throw(value)
            | Statement::Return(value) => Self::expression_contains_runtime_call(value),
            Statement::AssignMember {
                object,
                property,
                value,
            } => {
                Self::expression_contains_runtime_call(object)
                    || Self::expression_contains_runtime_call(property)
                    || Self::expression_contains_runtime_call(value)
            }
            Statement::Print { values } => {
                values.iter().any(Self::expression_contains_runtime_call)
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => {
                Self::expression_contains_runtime_call(condition)
                    || then_branch
                        .iter()
                        .any(Self::statement_contains_runtime_call)
                    || else_branch
                        .iter()
                        .any(Self::statement_contains_runtime_call)
            }
            Statement::Block { body } => body.iter().any(Self::statement_contains_runtime_call),
            _ => false,
        }
    }
}
