use super::*;

mod bindings;
mod control_transfer;
mod expression_statements;
mod structured_control;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_statement(
        &mut self,
        statement: &Statement,
    ) -> DirectResult<()> {
        match statement {
            Statement::Declaration { .. }
            | Statement::Block { .. }
            | Statement::Labeled { .. }
            | Statement::With { .. }
            | Statement::If { .. }
            | Statement::Try { .. }
            | Statement::Switch { .. } => self.emit_structured_statement(statement),
            Statement::Var { .. }
            | Statement::Let { .. }
            | Statement::Assign { .. }
            | Statement::AssignMember { .. } => self.emit_binding_statement(statement),
            Statement::Expression(..) | Statement::Print { .. } => {
                self.emit_expression_statement(statement)
            }
            Statement::While {
                condition,
                body,
                break_hook,
                labels,
            } => self.emit_while(condition, break_hook.as_ref(), labels, body),
            Statement::DoWhile {
                condition,
                body,
                break_hook,
                labels,
            } => self.emit_do_while(condition, break_hook.as_ref(), labels, body),
            Statement::For {
                init,
                condition,
                update,
                break_hook,
                labels,
                body,
                per_iteration_bindings,
            } => self.emit_for(
                labels,
                init,
                per_iteration_bindings,
                condition.as_ref(),
                update.as_ref(),
                break_hook.as_ref(),
                body,
            ),
            Statement::Break { .. }
            | Statement::Continue { .. }
            | Statement::Return(..)
            | Statement::Throw(..)
            | Statement::Yield { .. }
            | Statement::YieldDelegate { .. } => self.emit_control_transfer_statement(statement),
        }
    }
}
