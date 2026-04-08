use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn statement_creates_descriptor_binding_with_explicit_call_frame(
        &self,
        statement: &Statement,
        user_function: &UserFunction,
        arguments: &[Expression],
        this_expression: &Expression,
    ) -> bool {
        let call_arguments = Self::descriptor_binding_call_arguments(arguments);
        let arguments_binding = Self::descriptor_binding_arguments_expression(arguments);
        match statement {
            Statement::Var { value, .. } | Statement::Let { value, .. } => {
                let substituted = self.substitute_user_function_call_frame_bindings(
                    value,
                    user_function,
                    &call_arguments,
                    this_expression,
                    &arguments_binding,
                );
                self.resolve_descriptor_binding_from_expression(&substituted)
                    .is_some()
            }
            Statement::Block { body } => body.iter().any(|statement| {
                self.statement_creates_descriptor_binding_with_explicit_call_frame(
                    statement,
                    user_function,
                    arguments,
                    this_expression,
                )
            }),
            Statement::If {
                then_branch,
                else_branch,
                ..
            } => then_branch
                .iter()
                .chain(else_branch.iter())
                .any(|statement| {
                    self.statement_creates_descriptor_binding_with_explicit_call_frame(
                        statement,
                        user_function,
                        arguments,
                        this_expression,
                    )
                }),
            _ => false,
        }
    }
}
