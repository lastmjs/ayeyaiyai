use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn collect_static_direct_eval_assigned_nonlocal_names_from_statement(
        &self,
        statement: &Statement,
        current_function_name: Option<&str>,
        names: &mut HashSet<String>,
    ) {
        match statement {
            Statement::Declaration { body }
            | Statement::Block { body }
            | Statement::Labeled { body, .. } => {
                for statement in body {
                    self.collect_static_direct_eval_assigned_nonlocal_names_from_statement(
                        statement,
                        current_function_name,
                        names,
                    );
                }
            }
            Statement::Var { value, .. }
            | Statement::Let { value, .. }
            | Statement::Assign { value, .. }
            | Statement::Expression(value)
            | Statement::Throw(value)
            | Statement::Return(value)
            | Statement::Yield { value }
            | Statement::YieldDelegate { value } => {
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    value,
                    current_function_name,
                    names,
                );
            }
            Statement::Print { values } => {
                for value in values {
                    self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                        value,
                        current_function_name,
                        names,
                    );
                }
            }
            Statement::AssignMember {
                object,
                property,
                value,
            } => {
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    object,
                    current_function_name,
                    names,
                );
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    property,
                    current_function_name,
                    names,
                );
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    value,
                    current_function_name,
                    names,
                );
            }
            Statement::With { object, body } => {
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    object,
                    current_function_name,
                    names,
                );
                for statement in body {
                    self.collect_static_direct_eval_assigned_nonlocal_names_from_statement(
                        statement,
                        current_function_name,
                        names,
                    );
                }
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    condition,
                    current_function_name,
                    names,
                );
                for statement in then_branch {
                    self.collect_static_direct_eval_assigned_nonlocal_names_from_statement(
                        statement,
                        current_function_name,
                        names,
                    );
                }
                for statement in else_branch {
                    self.collect_static_direct_eval_assigned_nonlocal_names_from_statement(
                        statement,
                        current_function_name,
                        names,
                    );
                }
            }
            Statement::Try {
                body,
                catch_setup,
                catch_body,
                ..
            } => {
                for statement in body {
                    self.collect_static_direct_eval_assigned_nonlocal_names_from_statement(
                        statement,
                        current_function_name,
                        names,
                    );
                }
                for statement in catch_setup {
                    self.collect_static_direct_eval_assigned_nonlocal_names_from_statement(
                        statement,
                        current_function_name,
                        names,
                    );
                }
                for statement in catch_body {
                    self.collect_static_direct_eval_assigned_nonlocal_names_from_statement(
                        statement,
                        current_function_name,
                        names,
                    );
                }
            }
            Statement::Switch {
                discriminant,
                cases,
                ..
            } => {
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    discriminant,
                    current_function_name,
                    names,
                );
                for case in cases {
                    if let Some(test) = &case.test {
                        self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                            test,
                            current_function_name,
                            names,
                        );
                    }
                    for statement in &case.body {
                        self.collect_static_direct_eval_assigned_nonlocal_names_from_statement(
                            statement,
                            current_function_name,
                            names,
                        );
                    }
                }
            }
            Statement::For {
                init,
                condition,
                update,
                break_hook,
                body,
                ..
            } => {
                for statement in init {
                    self.collect_static_direct_eval_assigned_nonlocal_names_from_statement(
                        statement,
                        current_function_name,
                        names,
                    );
                }
                if let Some(condition) = condition {
                    self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                        condition,
                        current_function_name,
                        names,
                    );
                }
                if let Some(update) = update {
                    self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                        update,
                        current_function_name,
                        names,
                    );
                }
                if let Some(break_hook) = break_hook {
                    self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                        break_hook,
                        current_function_name,
                        names,
                    );
                }
                for statement in body {
                    self.collect_static_direct_eval_assigned_nonlocal_names_from_statement(
                        statement,
                        current_function_name,
                        names,
                    );
                }
            }
            Statement::While {
                condition,
                break_hook,
                body,
                ..
            }
            | Statement::DoWhile {
                condition,
                break_hook,
                body,
                ..
            } => {
                self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                    condition,
                    current_function_name,
                    names,
                );
                if let Some(break_hook) = break_hook {
                    self.collect_static_direct_eval_assigned_nonlocal_names_from_expression(
                        break_hook,
                        current_function_name,
                        names,
                    );
                }
                for statement in body {
                    self.collect_static_direct_eval_assigned_nonlocal_names_from_statement(
                        statement,
                        current_function_name,
                        names,
                    );
                }
            }
            Statement::Break { .. } | Statement::Continue { .. } => {}
        }
    }
}
