use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn substitute_capture_slot_bindings(
        &self,
        expression: &Expression,
        bindings: &BTreeMap<String, String>,
    ) -> Expression {
        match expression {
            Expression::Identifier(name) => bindings
                .get(name)
                .cloned()
                .map(Expression::Identifier)
                .unwrap_or_else(|| expression.clone()),
            Expression::Member { object, property } => Expression::Member {
                object: Box::new(self.substitute_capture_slot_bindings(object, bindings)),
                property: Box::new(self.substitute_capture_slot_bindings(property, bindings)),
            },
            Expression::Assign { name, value } => Expression::Assign {
                name: bindings.get(name).cloned().unwrap_or_else(|| name.clone()),
                value: Box::new(self.substitute_capture_slot_bindings(value, bindings)),
            },
            Expression::AssignMember {
                object,
                property,
                value,
            } => Expression::AssignMember {
                object: Box::new(self.substitute_capture_slot_bindings(object, bindings)),
                property: Box::new(self.substitute_capture_slot_bindings(property, bindings)),
                value: Box::new(self.substitute_capture_slot_bindings(value, bindings)),
            },
            Expression::AssignSuperMember { property, value } => Expression::AssignSuperMember {
                property: Box::new(self.substitute_capture_slot_bindings(property, bindings)),
                value: Box::new(self.substitute_capture_slot_bindings(value, bindings)),
            },
            Expression::Await(value) => Expression::Await(Box::new(
                self.substitute_capture_slot_bindings(value, bindings),
            )),
            Expression::EnumerateKeys(value) => Expression::EnumerateKeys(Box::new(
                self.substitute_capture_slot_bindings(value, bindings),
            )),
            Expression::GetIterator(value) => Expression::GetIterator(Box::new(
                self.substitute_capture_slot_bindings(value, bindings),
            )),
            Expression::IteratorClose(value) => Expression::IteratorClose(Box::new(
                self.substitute_capture_slot_bindings(value, bindings),
            )),
            Expression::Unary { op, expression } => Expression::Unary {
                op: *op,
                expression: Box::new(self.substitute_capture_slot_bindings(expression, bindings)),
            },
            Expression::Binary { op, left, right } => Expression::Binary {
                op: *op,
                left: Box::new(self.substitute_capture_slot_bindings(left, bindings)),
                right: Box::new(self.substitute_capture_slot_bindings(right, bindings)),
            },
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => Expression::Conditional {
                condition: Box::new(self.substitute_capture_slot_bindings(condition, bindings)),
                then_expression: Box::new(
                    self.substitute_capture_slot_bindings(then_expression, bindings),
                ),
                else_expression: Box::new(
                    self.substitute_capture_slot_bindings(else_expression, bindings),
                ),
            },
            Expression::Sequence(expressions) => Expression::Sequence(
                expressions
                    .iter()
                    .map(|expression| self.substitute_capture_slot_bindings(expression, bindings))
                    .collect(),
            ),
            Expression::Call { callee, arguments } => Expression::Call {
                callee: Box::new(self.substitute_capture_slot_bindings(callee, bindings)),
                arguments: arguments
                    .iter()
                    .map(|argument| match argument {
                        CallArgument::Expression(expression) => CallArgument::Expression(
                            self.substitute_capture_slot_bindings(expression, bindings),
                        ),
                        CallArgument::Spread(expression) => CallArgument::Spread(
                            self.substitute_capture_slot_bindings(expression, bindings),
                        ),
                    })
                    .collect(),
            },
            Expression::SuperCall { callee, arguments } => Expression::SuperCall {
                callee: Box::new(self.substitute_capture_slot_bindings(callee, bindings)),
                arguments: arguments
                    .iter()
                    .map(|argument| match argument {
                        CallArgument::Expression(expression) => CallArgument::Expression(
                            self.substitute_capture_slot_bindings(expression, bindings),
                        ),
                        CallArgument::Spread(expression) => CallArgument::Spread(
                            self.substitute_capture_slot_bindings(expression, bindings),
                        ),
                    })
                    .collect(),
            },
            Expression::New { callee, arguments } => Expression::New {
                callee: Box::new(self.substitute_capture_slot_bindings(callee, bindings)),
                arguments: arguments
                    .iter()
                    .map(|argument| match argument {
                        CallArgument::Expression(expression) => CallArgument::Expression(
                            self.substitute_capture_slot_bindings(expression, bindings),
                        ),
                        CallArgument::Spread(expression) => CallArgument::Spread(
                            self.substitute_capture_slot_bindings(expression, bindings),
                        ),
                    })
                    .collect(),
            },
            Expression::Array(elements) => Expression::Array(
                elements
                    .iter()
                    .map(|element| match element {
                        crate::ir::hir::ArrayElement::Expression(expression) => {
                            crate::ir::hir::ArrayElement::Expression(
                                self.substitute_capture_slot_bindings(expression, bindings),
                            )
                        }
                        crate::ir::hir::ArrayElement::Spread(expression) => {
                            crate::ir::hir::ArrayElement::Spread(
                                self.substitute_capture_slot_bindings(expression, bindings),
                            )
                        }
                    })
                    .collect(),
            ),
            Expression::Object(entries) => Expression::Object(
                entries
                    .iter()
                    .map(|entry| match entry {
                        crate::ir::hir::ObjectEntry::Data { key, value } => {
                            crate::ir::hir::ObjectEntry::Data {
                                key: self.substitute_capture_slot_bindings(key, bindings),
                                value: self.substitute_capture_slot_bindings(value, bindings),
                            }
                        }
                        crate::ir::hir::ObjectEntry::Getter { key, getter } => {
                            crate::ir::hir::ObjectEntry::Getter {
                                key: self.substitute_capture_slot_bindings(key, bindings),
                                getter: self.substitute_capture_slot_bindings(getter, bindings),
                            }
                        }
                        crate::ir::hir::ObjectEntry::Setter { key, setter } => {
                            crate::ir::hir::ObjectEntry::Setter {
                                key: self.substitute_capture_slot_bindings(key, bindings),
                                setter: self.substitute_capture_slot_bindings(setter, bindings),
                            }
                        }
                        crate::ir::hir::ObjectEntry::Spread(expression) => {
                            crate::ir::hir::ObjectEntry::Spread(
                                self.substitute_capture_slot_bindings(expression, bindings),
                            )
                        }
                    })
                    .collect(),
            ),
            Expression::Update { name, op, prefix } => Expression::Update {
                name: bindings.get(name).cloned().unwrap_or_else(|| name.clone()),
                op: *op,
                prefix: *prefix,
            },
            _ => expression.clone(),
        }
    }
}
