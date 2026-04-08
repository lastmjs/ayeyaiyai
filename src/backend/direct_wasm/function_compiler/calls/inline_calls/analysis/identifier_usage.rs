use super::*;

impl<'a> FunctionCompiler<'a> {
    fn expression_contains_identifier_callee_call(expression: &Expression) -> bool {
        match expression {
            Expression::Call { callee, arguments }
            | Expression::SuperCall { callee, arguments }
            | Expression::New { callee, arguments } => {
                matches!(callee.as_ref(), Expression::Identifier(_))
                    || Self::expression_contains_identifier_callee_call(callee)
                    || arguments.iter().any(|argument| match argument {
                        CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                            Self::expression_contains_identifier_callee_call(expression)
                        }
                    })
            }
            Expression::Member { object, property } => {
                Self::expression_contains_identifier_callee_call(object)
                    || Self::expression_contains_identifier_callee_call(property)
            }
            Expression::SuperMember { property } => {
                Self::expression_contains_identifier_callee_call(property)
            }
            Expression::Assign { value, .. }
            | Expression::Await(value)
            | Expression::EnumerateKeys(value)
            | Expression::GetIterator(value)
            | Expression::IteratorClose(value)
            | Expression::Unary {
                expression: value, ..
            } => Self::expression_contains_identifier_callee_call(value),
            Expression::AssignMember {
                object,
                property,
                value,
            } => {
                Self::expression_contains_identifier_callee_call(object)
                    || Self::expression_contains_identifier_callee_call(property)
                    || Self::expression_contains_identifier_callee_call(value)
            }
            Expression::AssignSuperMember { property, value } => {
                Self::expression_contains_identifier_callee_call(property)
                    || Self::expression_contains_identifier_callee_call(value)
            }
            Expression::Binary { left, right, .. } => {
                Self::expression_contains_identifier_callee_call(left)
                    || Self::expression_contains_identifier_callee_call(right)
            }
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                Self::expression_contains_identifier_callee_call(condition)
                    || Self::expression_contains_identifier_callee_call(then_expression)
                    || Self::expression_contains_identifier_callee_call(else_expression)
            }
            Expression::Sequence(expressions) => expressions
                .iter()
                .any(Self::expression_contains_identifier_callee_call),
            Expression::Array(elements) => elements.iter().any(|element| match element {
                ArrayElement::Expression(expression) | ArrayElement::Spread(expression) => {
                    Self::expression_contains_identifier_callee_call(expression)
                }
            }),
            Expression::Object(entries) => entries.iter().any(|entry| match entry {
                ObjectEntry::Data { key, value } => {
                    Self::expression_contains_identifier_callee_call(key)
                        || Self::expression_contains_identifier_callee_call(value)
                }
                ObjectEntry::Getter { key, getter }
                | ObjectEntry::Setter {
                    key,
                    setter: getter,
                } => {
                    Self::expression_contains_identifier_callee_call(key)
                        || Self::expression_contains_identifier_callee_call(getter)
                }
                ObjectEntry::Spread(expression) => {
                    Self::expression_contains_identifier_callee_call(expression)
                }
            }),
            _ => false,
        }
    }

    fn statement_contains_identifier_callee_call(statement: &Statement) -> bool {
        match statement {
            Statement::Var { value, .. }
            | Statement::Let { value, .. }
            | Statement::Assign { value, .. }
            | Statement::Expression(value)
            | Statement::Throw(value)
            | Statement::Return(value) => Self::expression_contains_identifier_callee_call(value),
            Statement::AssignMember {
                object,
                property,
                value,
            } => {
                Self::expression_contains_identifier_callee_call(object)
                    || Self::expression_contains_identifier_callee_call(property)
                    || Self::expression_contains_identifier_callee_call(value)
            }
            Statement::Print { values } => values
                .iter()
                .any(Self::expression_contains_identifier_callee_call),
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => {
                Self::expression_contains_identifier_callee_call(condition)
                    || then_branch
                        .iter()
                        .any(Self::statement_contains_identifier_callee_call)
                    || else_branch
                        .iter()
                        .any(Self::statement_contains_identifier_callee_call)
            }
            Statement::Block { body } => body
                .iter()
                .any(Self::statement_contains_identifier_callee_call),
            _ => false,
        }
    }

    pub(in crate::backend::direct_wasm) fn user_function_contains_identifier_callee_call(
        &self,
        user_function: &UserFunction,
    ) -> bool {
        self.resolve_registered_function_declaration(&user_function.name)
            .is_some_and(|function| {
                function
                    .body
                    .iter()
                    .any(Self::statement_contains_identifier_callee_call)
            })
    }

    fn statement_declares_local_binding(statement: &Statement) -> bool {
        match statement {
            Statement::Var { .. } | Statement::Let { .. } => true,
            Statement::Block { body } => body.iter().any(Self::statement_declares_local_binding),
            Statement::If {
                then_branch,
                else_branch,
                ..
            } => then_branch
                .iter()
                .chain(else_branch.iter())
                .any(Self::statement_declares_local_binding),
            _ => false,
        }
    }

    pub(in crate::backend::direct_wasm) fn user_function_contains_local_declaration(
        &self,
        user_function: &UserFunction,
    ) -> bool {
        self.resolve_registered_function_declaration(&user_function.name)
            .is_some_and(|function| {
                function
                    .body
                    .iter()
                    .any(Self::statement_declares_local_binding)
            })
    }
}
