use super::*;

impl<'a> FunctionCompiler<'a> {
    fn statement_contains_nested_async_generator_return_or_throw_chain(
        statement: &Statement,
    ) -> bool {
        match statement {
            Statement::Block { body } => body
                .iter()
                .any(Self::statement_contains_nested_async_generator_return_or_throw_chain),
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => {
                Self::expression_contains_nested_async_generator_return_or_throw_chain(condition)
                    || then_branch
                        .iter()
                        .any(Self::statement_contains_nested_async_generator_return_or_throw_chain)
                    || else_branch
                        .iter()
                        .any(Self::statement_contains_nested_async_generator_return_or_throw_chain)
            }
            Statement::Var { value, .. }
            | Statement::Let { value, .. }
            | Statement::Assign { value, .. }
            | Statement::Return(value)
            | Statement::Throw(value)
            | Statement::Expression(value) => {
                Self::expression_contains_nested_async_generator_return_or_throw_chain(value)
            }
            Statement::AssignMember {
                object,
                property,
                value,
            } => {
                Self::expression_contains_nested_async_generator_return_or_throw_chain(object)
                    || Self::expression_contains_nested_async_generator_return_or_throw_chain(
                        property,
                    )
                    || Self::expression_contains_nested_async_generator_return_or_throw_chain(value)
            }
            Statement::Print { values } => values
                .iter()
                .any(Self::expression_contains_nested_async_generator_return_or_throw_chain),
            _ => false,
        }
    }

    fn expression_contains_nested_async_generator_return_or_throw_chain(
        expression: &Expression,
    ) -> bool {
        match expression {
            Expression::Call { callee, arguments }
            | Expression::SuperCall { callee, arguments }
            | Expression::New { callee, arguments } => {
                if let Expression::Member { object, property } = callee.as_ref() {
                    if matches!(
                        property.as_ref(),
                        Expression::String(name) if name == "return" || name == "throw"
                    ) {
                        return true;
                    }
                    if Self::expression_contains_nested_async_generator_return_or_throw_chain(
                        object,
                    ) || Self::expression_contains_nested_async_generator_return_or_throw_chain(
                        property,
                    ) {
                        return true;
                    }
                } else if Self::expression_contains_nested_async_generator_return_or_throw_chain(
                    callee,
                ) {
                    return true;
                }
                arguments.iter().any(|argument| match argument {
                    CallArgument::Expression(value) | CallArgument::Spread(value) => {
                        Self::expression_contains_nested_async_generator_return_or_throw_chain(
                            value,
                        )
                    }
                })
            }
            Expression::Member { object, property } => {
                Self::expression_contains_nested_async_generator_return_or_throw_chain(object)
                    || Self::expression_contains_nested_async_generator_return_or_throw_chain(
                        property,
                    )
            }
            Expression::Assign { value, .. }
            | Expression::Await(value)
            | Expression::EnumerateKeys(value)
            | Expression::GetIterator(value)
            | Expression::IteratorClose(value) => {
                Self::expression_contains_nested_async_generator_return_or_throw_chain(value)
            }
            Expression::AssignMember {
                object,
                property,
                value,
            } => {
                Self::expression_contains_nested_async_generator_return_or_throw_chain(object)
                    || Self::expression_contains_nested_async_generator_return_or_throw_chain(
                        property,
                    )
                    || Self::expression_contains_nested_async_generator_return_or_throw_chain(value)
            }
            Expression::AssignSuperMember { property, value } => {
                Self::expression_contains_nested_async_generator_return_or_throw_chain(property)
                    || Self::expression_contains_nested_async_generator_return_or_throw_chain(value)
            }
            Expression::Unary { expression, .. } => {
                Self::expression_contains_nested_async_generator_return_or_throw_chain(expression)
            }
            Expression::Binary { left, right, .. } => {
                Self::expression_contains_nested_async_generator_return_or_throw_chain(left)
                    || Self::expression_contains_nested_async_generator_return_or_throw_chain(right)
            }
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                Self::expression_contains_nested_async_generator_return_or_throw_chain(condition)
                    || Self::expression_contains_nested_async_generator_return_or_throw_chain(
                        then_expression,
                    )
                    || Self::expression_contains_nested_async_generator_return_or_throw_chain(
                        else_expression,
                    )
            }
            Expression::Sequence(expressions) => expressions
                .iter()
                .any(Self::expression_contains_nested_async_generator_return_or_throw_chain),
            Expression::Array(elements) => elements.iter().any(|element| match element {
                ArrayElement::Expression(value) | ArrayElement::Spread(value) => {
                    Self::expression_contains_nested_async_generator_return_or_throw_chain(value)
                }
            }),
            Expression::Object(entries) => entries.iter().any(|entry| match entry {
                ObjectEntry::Data { key, value } => {
                    Self::expression_contains_nested_async_generator_return_or_throw_chain(key)
                        || Self::expression_contains_nested_async_generator_return_or_throw_chain(
                            value,
                        )
                }
                ObjectEntry::Getter { key, getter } => {
                    Self::expression_contains_nested_async_generator_return_or_throw_chain(key)
                        || Self::expression_contains_nested_async_generator_return_or_throw_chain(
                            getter,
                        )
                }
                ObjectEntry::Setter { key, setter } => {
                    Self::expression_contains_nested_async_generator_return_or_throw_chain(key)
                        || Self::expression_contains_nested_async_generator_return_or_throw_chain(
                            setter,
                        )
                }
                ObjectEntry::Spread(value) => {
                    Self::expression_contains_nested_async_generator_return_or_throw_chain(value)
                }
            }),
            _ => false,
        }
    }

    pub(in crate::backend::direct_wasm) fn promise_handler_requires_runtime_chain(
        &self,
        handler: &Expression,
    ) -> bool {
        let Some(user_function) = self.resolve_user_function_from_expression(handler) else {
            return false;
        };
        let Some(function) = self.resolve_registered_function_declaration(&user_function.name)
        else {
            return false;
        };
        function
            .body
            .iter()
            .any(Self::statement_contains_nested_async_generator_return_or_throw_chain)
    }

    pub(in crate::backend::direct_wasm) fn promise_member_call_requires_runtime_fallback(
        &self,
        object: &Expression,
        property: &Expression,
        _arguments: &[CallArgument],
    ) -> bool {
        let Expression::String(property_name) = property else {
            return false;
        };
        if property_name != "then" && property_name != "catch" {
            return false;
        }
        let Expression::Call { callee, .. } = object else {
            return false;
        };
        let Expression::Member {
            object: iterator_expression,
            property: iterator_property,
        } = callee.as_ref()
        else {
            return false;
        };
        matches!(
            iterator_property.as_ref(),
            Expression::String(name) if name == "return" || name == "throw"
        ) && self.is_async_generator_iterator_expression(iterator_expression)
    }

    pub(in crate::backend::direct_wasm) fn promise_argument_expression(
        &self,
        arguments: &[CallArgument],
        index: usize,
    ) -> Expression {
        match arguments.get(index) {
            Some(CallArgument::Expression(expression) | CallArgument::Spread(expression)) => {
                expression.clone()
            }
            None => Expression::Undefined,
        }
    }

    pub(in crate::backend::direct_wasm) fn call_is_promise_like_chain(
        expression: &Expression,
    ) -> bool {
        let Expression::Call { callee, .. } = expression else {
            return false;
        };
        let Expression::Member { property, .. } = callee.as_ref() else {
            return false;
        };
        matches!(
            property.as_ref(),
            Expression::String(name)
                if matches!(name.as_str(), "then" | "catch" | "next" | "return" | "throw")
        )
    }
}
