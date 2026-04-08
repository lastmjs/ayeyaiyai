use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn statement_creates_descriptor_binding_with_arguments(
        &self,
        statement: &Statement,
        user_function: &UserFunction,
        arguments: &[Expression],
    ) -> bool {
        match statement {
            Statement::Var { value, .. } | Statement::Let { value, .. } => {
                let call_arguments = Self::descriptor_binding_call_arguments(arguments);
                let substituted = self.substitute_user_function_argument_bindings(
                    value,
                    user_function,
                    &call_arguments,
                );
                self.resolve_descriptor_binding_from_expression(&substituted)
                    .is_some()
            }
            Statement::Block { body } => body.iter().any(|statement| {
                self.statement_creates_descriptor_binding_with_arguments(
                    statement,
                    user_function,
                    arguments,
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
                    self.statement_creates_descriptor_binding_with_arguments(
                        statement,
                        user_function,
                        arguments,
                    )
                }),
            _ => false,
        }
    }
}
