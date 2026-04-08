use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn substitute_call_frame_simple_expression(
        &self,
        expression: &Expression,
        user_function: &UserFunction,
        this_binding: &Expression,
        arguments_binding: &Expression,
    ) -> Option<Expression> {
        match expression {
            Expression::Member { object, property } => Some(Expression::Member {
                object: Box::new(self.substitute_call_frame_special_bindings(
                    object,
                    user_function,
                    this_binding,
                    arguments_binding,
                )),
                property: Box::new(self.substitute_call_frame_special_bindings(
                    property,
                    user_function,
                    this_binding,
                    arguments_binding,
                )),
            }),
            Expression::Assign { name, value } => Some(Expression::Assign {
                name: name.clone(),
                value: Box::new(self.substitute_call_frame_special_bindings(
                    value,
                    user_function,
                    this_binding,
                    arguments_binding,
                )),
            }),
            Expression::AssignMember {
                object,
                property,
                value,
            } => Some(Expression::AssignMember {
                object: Box::new(self.substitute_call_frame_special_bindings(
                    object,
                    user_function,
                    this_binding,
                    arguments_binding,
                )),
                property: Box::new(self.substitute_call_frame_special_bindings(
                    property,
                    user_function,
                    this_binding,
                    arguments_binding,
                )),
                value: Box::new(self.substitute_call_frame_special_bindings(
                    value,
                    user_function,
                    this_binding,
                    arguments_binding,
                )),
            }),
            Expression::AssignSuperMember { property, value } => {
                Some(Expression::AssignSuperMember {
                    property: Box::new(self.substitute_call_frame_special_bindings(
                        property,
                        user_function,
                        this_binding,
                        arguments_binding,
                    )),
                    value: Box::new(self.substitute_call_frame_special_bindings(
                        value,
                        user_function,
                        this_binding,
                        arguments_binding,
                    )),
                })
            }
            Expression::Await(value) => Some(Expression::Await(Box::new(
                self.substitute_call_frame_special_bindings(
                    value,
                    user_function,
                    this_binding,
                    arguments_binding,
                ),
            ))),
            Expression::EnumerateKeys(value) => Some(Expression::EnumerateKeys(Box::new(
                self.substitute_call_frame_special_bindings(
                    value,
                    user_function,
                    this_binding,
                    arguments_binding,
                ),
            ))),
            Expression::GetIterator(value) => Some(Expression::GetIterator(Box::new(
                self.substitute_call_frame_special_bindings(
                    value,
                    user_function,
                    this_binding,
                    arguments_binding,
                ),
            ))),
            Expression::IteratorClose(value) => Some(Expression::IteratorClose(Box::new(
                self.substitute_call_frame_special_bindings(
                    value,
                    user_function,
                    this_binding,
                    arguments_binding,
                ),
            ))),
            Expression::Unary { op, expression } => Some(Expression::Unary {
                op: *op,
                expression: Box::new(self.substitute_call_frame_special_bindings(
                    expression,
                    user_function,
                    this_binding,
                    arguments_binding,
                )),
            }),
            Expression::Binary { op, left, right } => Some(Expression::Binary {
                op: *op,
                left: Box::new(self.substitute_call_frame_special_bindings(
                    left,
                    user_function,
                    this_binding,
                    arguments_binding,
                )),
                right: Box::new(self.substitute_call_frame_special_bindings(
                    right,
                    user_function,
                    this_binding,
                    arguments_binding,
                )),
            }),
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => Some(Expression::Conditional {
                condition: Box::new(self.substitute_call_frame_special_bindings(
                    condition,
                    user_function,
                    this_binding,
                    arguments_binding,
                )),
                then_expression: Box::new(self.substitute_call_frame_special_bindings(
                    then_expression,
                    user_function,
                    this_binding,
                    arguments_binding,
                )),
                else_expression: Box::new(self.substitute_call_frame_special_bindings(
                    else_expression,
                    user_function,
                    this_binding,
                    arguments_binding,
                )),
            }),
            Expression::Sequence(expressions) => Some(Expression::Sequence(
                expressions
                    .iter()
                    .map(|expression| {
                        self.substitute_call_frame_special_bindings(
                            expression,
                            user_function,
                            this_binding,
                            arguments_binding,
                        )
                    })
                    .collect(),
            )),
            _ => None,
        }
    }
}
