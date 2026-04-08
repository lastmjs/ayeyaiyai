use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn statement_contains_try(statement: &Statement) -> bool {
        match statement {
            Statement::Try { .. } => true,
            Statement::Declaration { body }
            | Statement::Block { body }
            | Statement::Labeled { body, .. }
            | Statement::With { body, .. }
            | Statement::While { body, .. }
            | Statement::DoWhile { body, .. } => body.iter().any(Self::statement_contains_try),
            Statement::If {
                then_branch,
                else_branch,
                ..
            } => {
                then_branch.iter().any(Self::statement_contains_try)
                    || else_branch.iter().any(Self::statement_contains_try)
            }
            Statement::Switch { cases, .. } => cases
                .iter()
                .flat_map(|case| case.body.iter())
                .any(Self::statement_contains_try),
            Statement::For { init, body, .. } => {
                init.iter().any(Self::statement_contains_try)
                    || body.iter().any(Self::statement_contains_try)
            }
            Statement::Var { .. }
            | Statement::Let { .. }
            | Statement::Assign { .. }
            | Statement::AssignMember { .. }
            | Statement::Print { .. }
            | Statement::Expression(_)
            | Statement::Throw(_)
            | Statement::Return(_)
            | Statement::Break { .. }
            | Statement::Continue { .. }
            | Statement::Yield { .. }
            | Statement::YieldDelegate { .. } => false,
        }
    }

    pub(in crate::backend::direct_wasm) fn current_function_contains_try_statement(&self) -> bool {
        self.current_user_function_declaration()
            .is_some_and(|function| function.body.iter().any(Self::statement_contains_try))
    }

    pub(in crate::backend::direct_wasm) fn assertion_requires_runtime_same_value_fallback(
        &self,
    ) -> bool {
        self.current_function_contains_try_statement()
    }
}
