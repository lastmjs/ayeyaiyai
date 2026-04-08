use super::*;

impl<'a> FunctionCompiler<'a> {
    fn explicit_call_frame_inlineable_effect_statement(statement: &Statement) -> bool {
        match statement {
            Statement::Var { value, .. } | Statement::Let { value, .. } => {
                !expression_mentions_unsupported_explicit_call_frame_state(value)
            }
            Statement::Assign { value, .. } => {
                !expression_mentions_unsupported_explicit_call_frame_state(value)
            }
            Statement::Expression(Expression::Update { .. }) => true,
            Statement::Print { values } => values
                .iter()
                .all(|value| !expression_mentions_unsupported_explicit_call_frame_state(value)),
            Statement::Expression(expression) | Statement::Throw(expression) => {
                !expression_mentions_unsupported_explicit_call_frame_state(expression)
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => {
                !expression_mentions_unsupported_explicit_call_frame_state(condition)
                    && then_branch
                        .iter()
                        .all(Self::explicit_call_frame_inlineable_effect_statement)
                    && else_branch
                        .iter()
                        .all(Self::explicit_call_frame_inlineable_effect_statement)
            }
            Statement::Block { body } => body
                .iter()
                .all(Self::explicit_call_frame_inlineable_effect_statement),
            _ => false,
        }
    }

    pub(in crate::backend::direct_wasm) fn user_function_has_inlineable_terminal_body(
        &self,
        user_function: &UserFunction,
    ) -> bool {
        let Some(function) = self.resolve_registered_function_declaration(&user_function.name)
        else {
            return false;
        };
        let Some((terminal_statement, effect_statements)) = function.body.split_last() else {
            return false;
        };
        for statement in effect_statements {
            match statement {
                Statement::Assign { value, .. } => {
                    if !user_function.lexical_this && expression_mentions_call_frame_state(value) {
                        return false;
                    }
                }
                Statement::Expression(Expression::Update { .. }) => {}
                Statement::Print { .. } => {}
                Statement::Expression(expression) => {
                    if !user_function.lexical_this
                        && expression_mentions_call_frame_state(expression)
                    {
                        return false;
                    }
                }
                Statement::Block { body } if body.is_empty() => {}
                _ => return false,
            }
        }
        match terminal_statement {
            Statement::Return(expression) | Statement::Throw(expression) => {
                user_function.lexical_this || !expression_mentions_call_frame_state(expression)
            }
            Statement::Assign { value, .. } => {
                user_function.lexical_this || !expression_mentions_call_frame_state(value)
            }
            Statement::Expression(Expression::Update { .. }) => true,
            Statement::Print { values } => values.iter().all(|value| {
                user_function.lexical_this || !expression_mentions_call_frame_state(value)
            }),
            Statement::Block { body } if body.is_empty() => true,
            Statement::Expression(expression) => {
                user_function.lexical_this || !expression_mentions_call_frame_state(expression)
            }
            _ => false,
        }
    }

    pub(in crate::backend::direct_wasm) fn user_function_has_explicit_call_frame_inlineable_terminal_body(
        &self,
        user_function: &UserFunction,
    ) -> bool {
        if !self
            .user_function_parameter_iterator_consumption_indices(user_function)
            .is_empty()
        {
            return false;
        }
        let Some(function) = self.resolve_registered_function_declaration(&user_function.name)
        else {
            return false;
        };
        let Some((terminal_statement, effect_statements)) = function.body.split_last() else {
            return false;
        };
        for statement in effect_statements {
            if !Self::explicit_call_frame_inlineable_effect_statement(statement) {
                return false;
            }
        }
        match terminal_statement {
            Statement::Return(expression) | Statement::Throw(expression) => {
                !expression_mentions_unsupported_explicit_call_frame_state(expression)
            }
            Statement::Var { value, .. } | Statement::Let { value, .. } => {
                !expression_mentions_unsupported_explicit_call_frame_state(value)
            }
            Statement::Assign { value, .. } => {
                !expression_mentions_unsupported_explicit_call_frame_state(value)
            }
            Statement::Expression(Expression::Update { .. }) => true,
            Statement::Print { values } => values
                .iter()
                .all(|value| !expression_mentions_unsupported_explicit_call_frame_state(value)),
            Statement::Block { body } if body.is_empty() => true,
            Statement::Expression(expression) => {
                !expression_mentions_unsupported_explicit_call_frame_state(expression)
            }
            _ => false,
        }
    }

    pub(in crate::backend::direct_wasm) fn inline_argument_mentions_shadowed_implicit_global(
        &self,
        expression: &Expression,
    ) -> bool {
        match expression {
            Expression::Identifier(name) => {
                self.resolve_current_local_binding(name).is_some()
                    && self.backend.global_has_implicit_binding(name)
            }
            Expression::Member { object, property } => {
                self.inline_argument_mentions_shadowed_implicit_global(object)
                    || self.inline_argument_mentions_shadowed_implicit_global(property)
            }
            Expression::SuperMember { property } => {
                self.inline_argument_mentions_shadowed_implicit_global(property)
            }
            Expression::Assign { value, .. } => {
                self.inline_argument_mentions_shadowed_implicit_global(value)
            }
            Expression::AssignMember {
                object,
                property,
                value,
            } => {
                self.inline_argument_mentions_shadowed_implicit_global(object)
                    || self.inline_argument_mentions_shadowed_implicit_global(property)
                    || self.inline_argument_mentions_shadowed_implicit_global(value)
            }
            Expression::AssignSuperMember { property, value } => {
                self.inline_argument_mentions_shadowed_implicit_global(property)
                    || self.inline_argument_mentions_shadowed_implicit_global(value)
            }
            Expression::Await(value)
            | Expression::EnumerateKeys(value)
            | Expression::GetIterator(value)
            | Expression::IteratorClose(value)
            | Expression::Unary {
                expression: value, ..
            } => self.inline_argument_mentions_shadowed_implicit_global(value),
            Expression::Binary { left, right, .. } => {
                self.inline_argument_mentions_shadowed_implicit_global(left)
                    || self.inline_argument_mentions_shadowed_implicit_global(right)
            }
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                self.inline_argument_mentions_shadowed_implicit_global(condition)
                    || self.inline_argument_mentions_shadowed_implicit_global(then_expression)
                    || self.inline_argument_mentions_shadowed_implicit_global(else_expression)
            }
            Expression::Sequence(expressions) => expressions.iter().any(|expression| {
                self.inline_argument_mentions_shadowed_implicit_global(expression)
            }),
            Expression::Array(elements) => elements.iter().any(|element| match element {
                ArrayElement::Expression(expression) | ArrayElement::Spread(expression) => {
                    self.inline_argument_mentions_shadowed_implicit_global(expression)
                }
            }),
            Expression::Call { callee, arguments }
            | Expression::SuperCall { callee, arguments } => {
                self.inline_argument_mentions_shadowed_implicit_global(callee)
                    || arguments.iter().any(|argument| match argument {
                        CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                            self.inline_argument_mentions_shadowed_implicit_global(expression)
                        }
                    })
            }
            Expression::New { callee, arguments } => {
                self.inline_argument_mentions_shadowed_implicit_global(callee)
                    || arguments.iter().any(|argument| match argument {
                        CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                            self.inline_argument_mentions_shadowed_implicit_global(expression)
                        }
                    })
            }
            Expression::Object(entries) => entries.iter().any(|entry| match entry {
                crate::ir::hir::ObjectEntry::Data { key, value } => {
                    self.inline_argument_mentions_shadowed_implicit_global(key)
                        || self.inline_argument_mentions_shadowed_implicit_global(value)
                }
                crate::ir::hir::ObjectEntry::Getter { key, getter } => {
                    self.inline_argument_mentions_shadowed_implicit_global(key)
                        || self.inline_argument_mentions_shadowed_implicit_global(getter)
                }
                crate::ir::hir::ObjectEntry::Setter { key, setter } => {
                    self.inline_argument_mentions_shadowed_implicit_global(key)
                        || self.inline_argument_mentions_shadowed_implicit_global(setter)
                }
                crate::ir::hir::ObjectEntry::Spread(expression) => {
                    self.inline_argument_mentions_shadowed_implicit_global(expression)
                }
            }),
            Expression::NewTarget
            | Expression::This
            | Expression::Number(_)
            | Expression::BigInt(_)
            | Expression::String(_)
            | Expression::Bool(_)
            | Expression::Null
            | Expression::Undefined
            | Expression::Sent
            | Expression::Update { .. } => false,
        }
    }

    pub(in crate::backend::direct_wasm) fn user_function_references_captured_user_function(
        &self,
        user_function: &UserFunction,
    ) -> bool {
        if self
            .backend
            .function_registry
            .analysis
            .user_function_capture_bindings
            .is_empty()
        {
            return false;
        }
        let captured_user_function_names = self
            .backend
            .function_registry
            .analysis
            .user_function_capture_bindings
            .keys()
            .cloned()
            .collect::<HashSet<_>>();
        self.resolve_registered_function_declaration(&user_function.name)
            .is_some_and(|function| {
                function.body.iter().any(|statement| {
                    statement_references_user_function(statement, &captured_user_function_names)
                })
            })
    }
}
