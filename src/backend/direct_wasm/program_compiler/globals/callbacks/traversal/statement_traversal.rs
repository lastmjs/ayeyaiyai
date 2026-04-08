use super::*;

#[path = "statement_traversal/basic.rs"]
mod basic;
#[path = "statement_traversal/branches.rs"]
mod branches;
#[path = "statement_traversal/loops.rs"]
mod loops;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn collect_stateful_callback_bindings_from_statements(
        &self,
        statements: &[Statement],
        aliases: &HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
        value_bindings: &HashMap<String, Expression>,
        object_state: &HashMap<String, ObjectValueBinding>,
        overwrite_existing: bool,
    ) {
        for statement in statements {
            self.collect_stateful_callback_bindings_from_statement(
                statement,
                aliases,
                bindings,
                array_bindings,
                object_bindings,
                value_bindings,
                object_state,
                overwrite_existing,
            );
        }
    }

    pub(in crate::backend::direct_wasm) fn collect_stateful_callback_bindings_from_statement(
        &self,
        statement: &Statement,
        aliases: &HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
        value_bindings: &HashMap<String, Expression>,
        object_state: &HashMap<String, ObjectValueBinding>,
        overwrite_existing: bool,
    ) {
        match statement {
            Statement::Declaration { body }
            | Statement::Block { body }
            | Statement::Labeled { body, .. } => self
                .collect_stateful_callback_bindings_from_statements(
                    body,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                    value_bindings,
                    object_state,
                    overwrite_existing,
                ),
            Statement::Var { value, .. }
            | Statement::Let { value, .. }
            | Statement::Assign { value, .. }
            | Statement::Yield { value }
            | Statement::YieldDelegate { value }
            | Statement::Expression(value)
            | Statement::Throw(value)
            | Statement::Return(value) => self.collect_stateful_callback_bindings_from_expression(
                value,
                aliases,
                bindings,
                array_bindings,
                object_bindings,
                value_bindings,
                object_state,
                overwrite_existing,
            ),
            Statement::AssignMember {
                object,
                property,
                value,
            } => self.handle_assign_member_callback_statement(
                object,
                property,
                value,
                aliases,
                bindings,
                array_bindings,
                object_bindings,
                value_bindings,
                object_state,
                overwrite_existing,
            ),
            Statement::Print { values } => self.handle_print_callback_statement(
                values,
                aliases,
                bindings,
                array_bindings,
                object_bindings,
                value_bindings,
                object_state,
                overwrite_existing,
            ),
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => self.handle_if_callback_statement(
                condition,
                then_branch,
                else_branch,
                aliases,
                bindings,
                array_bindings,
                object_bindings,
                value_bindings,
                object_state,
                overwrite_existing,
            ),
            Statement::While {
                condition,
                body,
                break_hook,
                ..
            } => self.handle_while_callback_statement(
                condition,
                body,
                break_hook.as_ref(),
                aliases,
                bindings,
                array_bindings,
                object_bindings,
                value_bindings,
                object_state,
                overwrite_existing,
            ),
            Statement::DoWhile {
                condition,
                body,
                break_hook,
                ..
            } => self.handle_do_while_callback_statement(
                condition,
                body,
                break_hook.as_ref(),
                aliases,
                bindings,
                array_bindings,
                object_bindings,
                value_bindings,
                object_state,
                overwrite_existing,
            ),
            Statement::For {
                init,
                condition,
                update,
                body,
                break_hook,
                ..
            } => self.handle_for_callback_statement(
                init,
                condition.as_ref(),
                update.as_ref(),
                body,
                break_hook.as_ref(),
                aliases,
                bindings,
                array_bindings,
                object_bindings,
                value_bindings,
                object_state,
                overwrite_existing,
            ),
            Statement::With { object, body } => self.handle_with_callback_statement(
                object,
                body,
                aliases,
                bindings,
                array_bindings,
                object_bindings,
                value_bindings,
                object_state,
                overwrite_existing,
            ),
            Statement::Try {
                body,
                catch_setup,
                catch_body,
                ..
            } => self.handle_try_callback_statement(
                body,
                catch_setup,
                catch_body,
                aliases,
                bindings,
                array_bindings,
                object_bindings,
                value_bindings,
                object_state,
                overwrite_existing,
            ),
            Statement::Switch {
                discriminant,
                cases,
                ..
            } => self.handle_switch_callback_statement(
                discriminant,
                cases,
                aliases,
                bindings,
                array_bindings,
                object_bindings,
                value_bindings,
                object_state,
                overwrite_existing,
            ),
            Statement::Break { .. } | Statement::Continue { .. } => {}
        }
    }
}
