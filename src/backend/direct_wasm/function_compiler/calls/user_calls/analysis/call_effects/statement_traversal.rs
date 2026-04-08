use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn collect_statement_call_effect_nonlocal_bindings(
        &self,
        statement: &Statement,
        current_function_name: Option<&str>,
        names: &mut HashSet<String>,
        visited: &mut HashSet<String>,
    ) {
        match statement {
            Statement::Expression(expression) | Statement::Return(expression) => {
                self.collect_expression_call_effect_nonlocal_bindings(
                    expression,
                    current_function_name,
                    names,
                    visited,
                );
            }
            Statement::Throw(expression) => {
                self.collect_expression_call_effect_nonlocal_bindings(
                    expression,
                    current_function_name,
                    names,
                    visited,
                );
            }
            Statement::Var { value, .. }
            | Statement::Let { value, .. }
            | Statement::Assign { value, .. } => {
                self.collect_expression_call_effect_nonlocal_bindings(
                    value,
                    current_function_name,
                    names,
                    visited,
                );
            }
            Statement::AssignMember {
                object,
                property,
                value,
            } => {
                self.collect_expression_call_effect_nonlocal_bindings(
                    &Expression::AssignMember {
                        object: Box::new(object.clone()),
                        property: Box::new(property.clone()),
                        value: Box::new(value.clone()),
                    },
                    current_function_name,
                    names,
                    visited,
                );
            }
            Statement::Declaration { body } | Statement::Block { body } => {
                for statement in body {
                    self.collect_statement_call_effect_nonlocal_bindings(
                        statement,
                        current_function_name,
                        names,
                        visited,
                    );
                }
            }
            Statement::Labeled { body, .. } => {
                for statement in body {
                    self.collect_statement_call_effect_nonlocal_bindings(
                        statement,
                        current_function_name,
                        names,
                        visited,
                    );
                }
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.collect_expression_call_effect_nonlocal_bindings(
                    condition,
                    current_function_name,
                    names,
                    visited,
                );
                for statement in then_branch {
                    self.collect_statement_call_effect_nonlocal_bindings(
                        statement,
                        current_function_name,
                        names,
                        visited,
                    );
                }
                for statement in else_branch {
                    self.collect_statement_call_effect_nonlocal_bindings(
                        statement,
                        current_function_name,
                        names,
                        visited,
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
                    self.collect_statement_call_effect_nonlocal_bindings(
                        statement,
                        current_function_name,
                        names,
                        visited,
                    );
                }
                for statement in catch_setup {
                    self.collect_statement_call_effect_nonlocal_bindings(
                        statement,
                        current_function_name,
                        names,
                        visited,
                    );
                }
                for statement in catch_body {
                    self.collect_statement_call_effect_nonlocal_bindings(
                        statement,
                        current_function_name,
                        names,
                        visited,
                    );
                }
            }
            Statement::Switch {
                discriminant,
                cases,
                ..
            } => {
                self.collect_expression_call_effect_nonlocal_bindings(
                    discriminant,
                    current_function_name,
                    names,
                    visited,
                );
                for case in cases {
                    if let Some(test) = &case.test {
                        self.collect_expression_call_effect_nonlocal_bindings(
                            test,
                            current_function_name,
                            names,
                            visited,
                        );
                    }
                    for statement in &case.body {
                        self.collect_statement_call_effect_nonlocal_bindings(
                            statement,
                            current_function_name,
                            names,
                            visited,
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
                    self.collect_statement_call_effect_nonlocal_bindings(
                        statement,
                        current_function_name,
                        names,
                        visited,
                    );
                }
                if let Some(condition) = condition {
                    self.collect_expression_call_effect_nonlocal_bindings(
                        condition,
                        current_function_name,
                        names,
                        visited,
                    );
                }
                if let Some(update) = update {
                    self.collect_expression_call_effect_nonlocal_bindings(
                        update,
                        current_function_name,
                        names,
                        visited,
                    );
                }
                if let Some(break_hook) = break_hook {
                    self.collect_expression_call_effect_nonlocal_bindings(
                        break_hook,
                        current_function_name,
                        names,
                        visited,
                    );
                }
                for statement in body {
                    self.collect_statement_call_effect_nonlocal_bindings(
                        statement,
                        current_function_name,
                        names,
                        visited,
                    );
                }
            }
            Statement::With { object, body } => {
                self.collect_expression_call_effect_nonlocal_bindings(
                    object,
                    current_function_name,
                    names,
                    visited,
                );
                for statement in body {
                    self.collect_statement_call_effect_nonlocal_bindings(
                        statement,
                        current_function_name,
                        names,
                        visited,
                    );
                }
            }
            Statement::While {
                condition,
                break_hook,
                body,
                ..
            } => {
                self.collect_expression_call_effect_nonlocal_bindings(
                    condition,
                    current_function_name,
                    names,
                    visited,
                );
                if let Some(break_hook) = break_hook {
                    self.collect_expression_call_effect_nonlocal_bindings(
                        break_hook,
                        current_function_name,
                        names,
                        visited,
                    );
                }
                for statement in body {
                    self.collect_statement_call_effect_nonlocal_bindings(
                        statement,
                        current_function_name,
                        names,
                        visited,
                    );
                }
            }
            Statement::DoWhile {
                condition,
                break_hook,
                body,
                ..
            } => {
                self.collect_expression_call_effect_nonlocal_bindings(
                    condition,
                    current_function_name,
                    names,
                    visited,
                );
                if let Some(break_hook) = break_hook {
                    self.collect_expression_call_effect_nonlocal_bindings(
                        break_hook,
                        current_function_name,
                        names,
                        visited,
                    );
                }
                for statement in body {
                    self.collect_statement_call_effect_nonlocal_bindings(
                        statement,
                        current_function_name,
                        names,
                        visited,
                    );
                }
            }
            Statement::Print { values } => {
                for value in values {
                    self.collect_expression_call_effect_nonlocal_bindings(
                        value,
                        current_function_name,
                        names,
                        visited,
                    );
                }
            }
            Statement::Yield { value } | Statement::YieldDelegate { value } => {
                self.collect_expression_call_effect_nonlocal_bindings(
                    value,
                    current_function_name,
                    names,
                    visited,
                );
            }
            Statement::Break { .. } | Statement::Continue { .. } => {}
        }
    }
}
