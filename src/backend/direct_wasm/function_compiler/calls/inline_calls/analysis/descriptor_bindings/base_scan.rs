use super::*;

impl<'a> FunctionCompiler<'a> {
    fn statement_creates_descriptor_binding(&self, statement: &Statement) -> bool {
        match statement {
            Statement::Var { value, .. } | Statement::Let { value, .. } => self
                .resolve_descriptor_binding_from_expression(value)
                .is_some(),
            Statement::Block { body } => body
                .iter()
                .any(|statement| self.statement_creates_descriptor_binding(statement)),
            Statement::If {
                then_branch,
                else_branch,
                ..
            } => then_branch
                .iter()
                .chain(else_branch.iter())
                .any(|statement| self.statement_creates_descriptor_binding(statement)),
            _ => false,
        }
    }

    fn user_function_creates_descriptor_binding(&self, user_function: &UserFunction) -> bool {
        self.resolve_registered_function_declaration(&user_function.name)
            .is_some_and(|function| {
                function
                    .body
                    .iter()
                    .any(|statement| self.statement_creates_descriptor_binding(statement))
            })
    }
}
