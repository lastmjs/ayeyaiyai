use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn substitute_call_frame_aggregate_expression(
        &self,
        expression: &Expression,
        user_function: &UserFunction,
        this_binding: &Expression,
        arguments_binding: &Expression,
    ) -> Option<Expression> {
        match expression {
            Expression::Call { callee, arguments } => Some(Expression::Call {
                callee: Box::new(self.substitute_call_frame_special_bindings(
                    callee,
                    user_function,
                    this_binding,
                    arguments_binding,
                )),
                arguments: arguments
                    .iter()
                    .map(|argument| match argument {
                        CallArgument::Expression(expression) => {
                            CallArgument::Expression(self.substitute_call_frame_special_bindings(
                                expression,
                                user_function,
                                this_binding,
                                arguments_binding,
                            ))
                        }
                        CallArgument::Spread(expression) => {
                            CallArgument::Spread(self.substitute_call_frame_special_bindings(
                                expression,
                                user_function,
                                this_binding,
                                arguments_binding,
                            ))
                        }
                    })
                    .collect(),
            }),
            Expression::SuperCall { callee, arguments } => Some(Expression::SuperCall {
                callee: Box::new(self.substitute_call_frame_special_bindings(
                    callee,
                    user_function,
                    this_binding,
                    arguments_binding,
                )),
                arguments: arguments
                    .iter()
                    .map(|argument| match argument {
                        CallArgument::Expression(expression) => {
                            CallArgument::Expression(self.substitute_call_frame_special_bindings(
                                expression,
                                user_function,
                                this_binding,
                                arguments_binding,
                            ))
                        }
                        CallArgument::Spread(expression) => {
                            CallArgument::Spread(self.substitute_call_frame_special_bindings(
                                expression,
                                user_function,
                                this_binding,
                                arguments_binding,
                            ))
                        }
                    })
                    .collect(),
            }),
            Expression::New { callee, arguments } => Some(Expression::New {
                callee: Box::new(self.substitute_call_frame_special_bindings(
                    callee,
                    user_function,
                    this_binding,
                    arguments_binding,
                )),
                arguments: arguments
                    .iter()
                    .map(|argument| match argument {
                        CallArgument::Expression(expression) => {
                            CallArgument::Expression(self.substitute_call_frame_special_bindings(
                                expression,
                                user_function,
                                this_binding,
                                arguments_binding,
                            ))
                        }
                        CallArgument::Spread(expression) => {
                            CallArgument::Spread(self.substitute_call_frame_special_bindings(
                                expression,
                                user_function,
                                this_binding,
                                arguments_binding,
                            ))
                        }
                    })
                    .collect(),
            }),
            Expression::Array(elements) => Some(Expression::Array(
                elements
                    .iter()
                    .map(|element| match element {
                        crate::ir::hir::ArrayElement::Expression(expression) => {
                            crate::ir::hir::ArrayElement::Expression(
                                self.substitute_call_frame_special_bindings(
                                    expression,
                                    user_function,
                                    this_binding,
                                    arguments_binding,
                                ),
                            )
                        }
                        crate::ir::hir::ArrayElement::Spread(expression) => {
                            crate::ir::hir::ArrayElement::Spread(
                                self.substitute_call_frame_special_bindings(
                                    expression,
                                    user_function,
                                    this_binding,
                                    arguments_binding,
                                ),
                            )
                        }
                    })
                    .collect(),
            )),
            Expression::Object(entries) => Some(Expression::Object(
                entries
                    .iter()
                    .map(|entry| match entry {
                        crate::ir::hir::ObjectEntry::Data { key, value } => {
                            crate::ir::hir::ObjectEntry::Data {
                                key: self.substitute_call_frame_special_bindings(
                                    key,
                                    user_function,
                                    this_binding,
                                    arguments_binding,
                                ),
                                value: self.substitute_call_frame_special_bindings(
                                    value,
                                    user_function,
                                    this_binding,
                                    arguments_binding,
                                ),
                            }
                        }
                        crate::ir::hir::ObjectEntry::Getter { key, getter } => {
                            crate::ir::hir::ObjectEntry::Getter {
                                key: self.substitute_call_frame_special_bindings(
                                    key,
                                    user_function,
                                    this_binding,
                                    arguments_binding,
                                ),
                                getter: self.substitute_call_frame_special_bindings(
                                    getter,
                                    user_function,
                                    this_binding,
                                    arguments_binding,
                                ),
                            }
                        }
                        crate::ir::hir::ObjectEntry::Setter { key, setter } => {
                            crate::ir::hir::ObjectEntry::Setter {
                                key: self.substitute_call_frame_special_bindings(
                                    key,
                                    user_function,
                                    this_binding,
                                    arguments_binding,
                                ),
                                setter: self.substitute_call_frame_special_bindings(
                                    setter,
                                    user_function,
                                    this_binding,
                                    arguments_binding,
                                ),
                            }
                        }
                        crate::ir::hir::ObjectEntry::Spread(expression) => {
                            crate::ir::hir::ObjectEntry::Spread(
                                self.substitute_call_frame_special_bindings(
                                    expression,
                                    user_function,
                                    this_binding,
                                    arguments_binding,
                                ),
                            )
                        }
                    })
                    .collect(),
            )),
            Expression::SuperMember { property } => Some(Expression::SuperMember {
                property: Box::new(self.substitute_call_frame_special_bindings(
                    property,
                    user_function,
                    this_binding,
                    arguments_binding,
                )),
            }),
            _ => None,
        }
    }
}
