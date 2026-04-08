use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn invalidate_active_inline_local_descriptor_bindings_except(
        &mut self,
        inline_local_bindings: &[String],
        preserved_source_name: Option<&str>,
    ) {
        for name in inline_local_bindings {
            if preserved_source_name.is_some_and(|preserved| preserved == name) {
                continue;
            }
            if let Some(active_bindings) = self
                .state
                .emission
                .lexical_scopes
                .active_scoped_lexical_bindings
                .get(name)
                && let Some(active_binding) = active_bindings.last().cloned()
            {
                self.state
                    .speculation
                    .static_semantics
                    .objects
                    .local_descriptor_bindings
                    .remove(&active_binding);
            }
        }
    }

    pub(in crate::backend::direct_wasm) fn sanitize_snapshot_await_marker_expression(
        &self,
        expression: &Expression,
    ) -> Expression {
        match expression {
            Expression::Identifier(name) if name == SNAPSHOT_AWAIT_RESOLVE_BINDING => {
                Expression::Identifier("Boolean".to_string())
            }
            Expression::Identifier(name) if name == SNAPSHOT_AWAIT_REJECT_BINDING => {
                Expression::Identifier("Boolean".to_string())
            }
            Expression::Member { object, property } => Expression::Member {
                object: Box::new(self.sanitize_snapshot_await_marker_expression(object)),
                property: Box::new(self.sanitize_snapshot_await_marker_expression(property)),
            },
            Expression::Assign { name, value } => Expression::Assign {
                name: name.clone(),
                value: Box::new(self.sanitize_snapshot_await_marker_expression(value)),
            },
            Expression::AssignMember {
                object,
                property,
                value,
            } => Expression::AssignMember {
                object: Box::new(self.sanitize_snapshot_await_marker_expression(object)),
                property: Box::new(self.sanitize_snapshot_await_marker_expression(property)),
                value: Box::new(self.sanitize_snapshot_await_marker_expression(value)),
            },
            Expression::AssignSuperMember { property, value } => Expression::AssignSuperMember {
                property: Box::new(self.sanitize_snapshot_await_marker_expression(property)),
                value: Box::new(self.sanitize_snapshot_await_marker_expression(value)),
            },
            Expression::Await(value) => Expression::Await(Box::new(
                self.sanitize_snapshot_await_marker_expression(value),
            )),
            Expression::EnumerateKeys(value) => Expression::EnumerateKeys(Box::new(
                self.sanitize_snapshot_await_marker_expression(value),
            )),
            Expression::GetIterator(value) => Expression::GetIterator(Box::new(
                self.sanitize_snapshot_await_marker_expression(value),
            )),
            Expression::IteratorClose(value) => Expression::IteratorClose(Box::new(
                self.sanitize_snapshot_await_marker_expression(value),
            )),
            Expression::Unary { op, expression } => Expression::Unary {
                op: *op,
                expression: Box::new(self.sanitize_snapshot_await_marker_expression(expression)),
            },
            Expression::Binary { op, left, right } => Expression::Binary {
                op: *op,
                left: Box::new(self.sanitize_snapshot_await_marker_expression(left)),
                right: Box::new(self.sanitize_snapshot_await_marker_expression(right)),
            },
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => Expression::Conditional {
                condition: Box::new(self.sanitize_snapshot_await_marker_expression(condition)),
                then_expression: Box::new(
                    self.sanitize_snapshot_await_marker_expression(then_expression),
                ),
                else_expression: Box::new(
                    self.sanitize_snapshot_await_marker_expression(else_expression),
                ),
            },
            Expression::Sequence(expressions) => Expression::Sequence(
                expressions
                    .iter()
                    .map(|expression| self.sanitize_snapshot_await_marker_expression(expression))
                    .collect(),
            ),
            Expression::Call { callee, arguments } => Expression::Call {
                callee: Box::new(self.sanitize_snapshot_await_marker_expression(callee)),
                arguments: arguments
                    .iter()
                    .map(|argument| match argument {
                        CallArgument::Expression(expression) => CallArgument::Expression(
                            self.sanitize_snapshot_await_marker_expression(expression),
                        ),
                        CallArgument::Spread(expression) => CallArgument::Spread(
                            self.sanitize_snapshot_await_marker_expression(expression),
                        ),
                    })
                    .collect(),
            },
            Expression::SuperCall { callee, arguments } => Expression::SuperCall {
                callee: Box::new(self.sanitize_snapshot_await_marker_expression(callee)),
                arguments: arguments
                    .iter()
                    .map(|argument| match argument {
                        CallArgument::Expression(expression) => CallArgument::Expression(
                            self.sanitize_snapshot_await_marker_expression(expression),
                        ),
                        CallArgument::Spread(expression) => CallArgument::Spread(
                            self.sanitize_snapshot_await_marker_expression(expression),
                        ),
                    })
                    .collect(),
            },
            Expression::New { callee, arguments } => Expression::New {
                callee: Box::new(self.sanitize_snapshot_await_marker_expression(callee)),
                arguments: arguments
                    .iter()
                    .map(|argument| match argument {
                        CallArgument::Expression(expression) => CallArgument::Expression(
                            self.sanitize_snapshot_await_marker_expression(expression),
                        ),
                        CallArgument::Spread(expression) => CallArgument::Spread(
                            self.sanitize_snapshot_await_marker_expression(expression),
                        ),
                    })
                    .collect(),
            },
            Expression::Array(elements) => Expression::Array(
                elements
                    .iter()
                    .map(|element| match element {
                        ArrayElement::Expression(expression) => ArrayElement::Expression(
                            self.sanitize_snapshot_await_marker_expression(expression),
                        ),
                        ArrayElement::Spread(expression) => ArrayElement::Spread(
                            self.sanitize_snapshot_await_marker_expression(expression),
                        ),
                    })
                    .collect(),
            ),
            Expression::Object(entries) => Expression::Object(
                entries
                    .iter()
                    .map(|entry| match entry {
                        ObjectEntry::Data { key, value } => ObjectEntry::Data {
                            key: self.sanitize_snapshot_await_marker_expression(key),
                            value: self.sanitize_snapshot_await_marker_expression(value),
                        },
                        ObjectEntry::Getter { key, getter } => ObjectEntry::Getter {
                            key: self.sanitize_snapshot_await_marker_expression(key),
                            getter: self.sanitize_snapshot_await_marker_expression(getter),
                        },
                        ObjectEntry::Setter { key, setter } => ObjectEntry::Setter {
                            key: self.sanitize_snapshot_await_marker_expression(key),
                            setter: self.sanitize_snapshot_await_marker_expression(setter),
                        },
                        ObjectEntry::Spread(expression) => ObjectEntry::Spread(
                            self.sanitize_snapshot_await_marker_expression(expression),
                        ),
                    })
                    .collect(),
            ),
            _ => expression.clone(),
        }
    }
}
