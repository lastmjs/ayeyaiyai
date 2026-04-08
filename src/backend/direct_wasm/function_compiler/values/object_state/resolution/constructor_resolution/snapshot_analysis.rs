use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn statement_contains_static_constructor_snapshot_call(
        statement: &Statement,
    ) -> bool {
        match statement {
            Statement::Declaration { body }
            | Statement::Block { body }
            | Statement::Labeled { body, .. }
            | Statement::With { body, .. } => body
                .iter()
                .any(Self::statement_contains_static_constructor_snapshot_call),
            Statement::Expression(expression)
            | Statement::Return(expression)
            | Statement::Throw(expression)
            | Statement::Yield { value: expression }
            | Statement::YieldDelegate { value: expression } => {
                Self::expression_contains_static_constructor_snapshot_call(expression)
            }
            Statement::Var { value, .. }
            | Statement::Let { value, .. }
            | Statement::Assign { value, .. } => {
                Self::expression_contains_static_constructor_snapshot_call(value)
            }
            Statement::AssignMember {
                object,
                property,
                value,
            } => {
                Self::expression_contains_static_constructor_snapshot_call(object)
                    || Self::expression_contains_static_constructor_snapshot_call(property)
                    || Self::expression_contains_static_constructor_snapshot_call(value)
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => {
                Self::expression_contains_static_constructor_snapshot_call(condition)
                    || then_branch
                        .iter()
                        .any(Self::statement_contains_static_constructor_snapshot_call)
                    || else_branch
                        .iter()
                        .any(Self::statement_contains_static_constructor_snapshot_call)
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
                Self::expression_contains_static_constructor_snapshot_call(condition)
                    || break_hook
                        .as_ref()
                        .is_some_and(Self::expression_contains_static_constructor_snapshot_call)
                    || body
                        .iter()
                        .any(Self::statement_contains_static_constructor_snapshot_call)
            }
            Statement::For {
                init,
                condition,
                update,
                break_hook,
                body,
                ..
            } => {
                init.iter()
                    .any(Self::statement_contains_static_constructor_snapshot_call)
                    || condition
                        .as_ref()
                        .is_some_and(Self::expression_contains_static_constructor_snapshot_call)
                    || update
                        .as_ref()
                        .is_some_and(Self::expression_contains_static_constructor_snapshot_call)
                    || break_hook
                        .as_ref()
                        .is_some_and(Self::expression_contains_static_constructor_snapshot_call)
                    || body
                        .iter()
                        .any(Self::statement_contains_static_constructor_snapshot_call)
            }
            Statement::Try {
                body,
                catch_setup,
                catch_body,
                ..
            } => {
                body.iter()
                    .any(Self::statement_contains_static_constructor_snapshot_call)
                    || catch_setup
                        .iter()
                        .any(Self::statement_contains_static_constructor_snapshot_call)
                    || catch_body
                        .iter()
                        .any(Self::statement_contains_static_constructor_snapshot_call)
            }
            Statement::Switch {
                discriminant,
                cases,
                ..
            } => {
                Self::expression_contains_static_constructor_snapshot_call(discriminant)
                    || cases.iter().any(|case| {
                        case.test
                            .as_ref()
                            .is_some_and(Self::expression_contains_static_constructor_snapshot_call)
                            || case
                                .body
                                .iter()
                                .any(Self::statement_contains_static_constructor_snapshot_call)
                    })
            }
            Statement::Print { values } => values
                .iter()
                .any(Self::expression_contains_static_constructor_snapshot_call),
            Statement::Break { .. } | Statement::Continue { .. } => false,
        }
    }

    pub(in crate::backend::direct_wasm) fn expression_contains_static_constructor_snapshot_call(
        expression: &Expression,
    ) -> bool {
        match expression {
            Expression::Call { callee, arguments }
            | Expression::SuperCall { callee, arguments }
            | Expression::New { callee, arguments } => {
                let _ = (callee, arguments);
                true
            }
            Expression::Member { object, property } => {
                Self::expression_contains_static_constructor_snapshot_call(object)
                    || Self::expression_contains_static_constructor_snapshot_call(property)
            }
            Expression::SuperMember { property } => {
                Self::expression_contains_static_constructor_snapshot_call(property)
            }
            Expression::Assign { value, .. }
            | Expression::Await(value)
            | Expression::EnumerateKeys(value)
            | Expression::GetIterator(value)
            | Expression::IteratorClose(value)
            | Expression::Unary {
                expression: value, ..
            } => Self::expression_contains_static_constructor_snapshot_call(value),
            Expression::AssignMember {
                object,
                property,
                value,
            } => {
                Self::expression_contains_static_constructor_snapshot_call(object)
                    || Self::expression_contains_static_constructor_snapshot_call(property)
                    || Self::expression_contains_static_constructor_snapshot_call(value)
            }
            Expression::AssignSuperMember { property, value } => {
                Self::expression_contains_static_constructor_snapshot_call(property)
                    || Self::expression_contains_static_constructor_snapshot_call(value)
            }
            Expression::Binary { left, right, .. } => {
                Self::expression_contains_static_constructor_snapshot_call(left)
                    || Self::expression_contains_static_constructor_snapshot_call(right)
            }
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                Self::expression_contains_static_constructor_snapshot_call(condition)
                    || Self::expression_contains_static_constructor_snapshot_call(then_expression)
                    || Self::expression_contains_static_constructor_snapshot_call(else_expression)
            }
            Expression::Sequence(expressions) => expressions
                .iter()
                .any(Self::expression_contains_static_constructor_snapshot_call),
            Expression::Array(elements) => elements.iter().any(|element| match element {
                ArrayElement::Expression(expression) | ArrayElement::Spread(expression) => {
                    Self::expression_contains_static_constructor_snapshot_call(expression)
                }
            }),
            Expression::Object(entries) => entries.iter().any(|entry| match entry {
                ObjectEntry::Data { key, value } => {
                    Self::expression_contains_static_constructor_snapshot_call(key)
                        || Self::expression_contains_static_constructor_snapshot_call(value)
                }
                ObjectEntry::Getter { key, getter } => {
                    Self::expression_contains_static_constructor_snapshot_call(key)
                        || Self::expression_contains_static_constructor_snapshot_call(getter)
                }
                ObjectEntry::Setter { key, setter } => {
                    Self::expression_contains_static_constructor_snapshot_call(key)
                        || Self::expression_contains_static_constructor_snapshot_call(setter)
                }
                ObjectEntry::Spread(expression) => {
                    Self::expression_contains_static_constructor_snapshot_call(expression)
                }
            }),
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
            | Expression::Update { .. } => false,
        }
    }
}
