use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn register_static_eval_functions_in_statement(
        &mut self,
        statement: &Statement,
        current_function_name: Option<&str>,
    ) -> DirectResult<()> {
        match statement {
            Statement::Declaration { body }
            | Statement::Block { body }
            | Statement::Labeled { body, .. } => {
                self.register_static_eval_functions_in_statements(body, current_function_name)?;
            }
            Statement::Var { value, .. }
            | Statement::Let { value, .. }
            | Statement::Assign { value, .. }
            | Statement::Expression(value)
            | Statement::Throw(value)
            | Statement::Return(value)
            | Statement::Yield { value }
            | Statement::YieldDelegate { value } => {
                self.register_static_eval_functions_in_expression(value, current_function_name)?;
            }
            Statement::Print { values } => {
                for value in values {
                    self.register_static_eval_functions_in_expression(
                        value,
                        current_function_name,
                    )?;
                }
            }
            Statement::AssignMember {
                object,
                property,
                value,
            } => {
                self.register_static_eval_functions_in_expression(object, current_function_name)?;
                self.register_static_eval_functions_in_expression(property, current_function_name)?;
                self.register_static_eval_functions_in_expression(value, current_function_name)?;
            }
            Statement::With { object, body } => {
                self.register_static_eval_functions_in_expression(object, current_function_name)?;
                self.register_static_eval_functions_in_statements(body, current_function_name)?;
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.register_static_eval_functions_in_expression(
                    condition,
                    current_function_name,
                )?;
                self.register_static_eval_functions_in_statements(
                    then_branch,
                    current_function_name,
                )?;
                self.register_static_eval_functions_in_statements(
                    else_branch,
                    current_function_name,
                )?;
            }
            Statement::Try {
                body,
                catch_setup,
                catch_body,
                ..
            } => {
                self.register_static_eval_functions_in_statements(body, current_function_name)?;
                self.register_static_eval_functions_in_statements(
                    catch_setup,
                    current_function_name,
                )?;
                self.register_static_eval_functions_in_statements(
                    catch_body,
                    current_function_name,
                )?;
            }
            Statement::Switch {
                discriminant,
                cases,
                ..
            } => {
                self.register_static_eval_functions_in_expression(
                    discriminant,
                    current_function_name,
                )?;
                for case in cases {
                    if let Some(test) = &case.test {
                        self.register_static_eval_functions_in_expression(
                            test,
                            current_function_name,
                        )?;
                    }
                    self.register_static_eval_functions_in_statements(
                        &case.body,
                        current_function_name,
                    )?;
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
                self.register_static_eval_functions_in_statements(init, current_function_name)?;
                if let Some(condition) = condition {
                    self.register_static_eval_functions_in_expression(
                        condition,
                        current_function_name,
                    )?;
                }
                if let Some(update) = update {
                    self.register_static_eval_functions_in_expression(
                        update,
                        current_function_name,
                    )?;
                }
                if let Some(break_hook) = break_hook {
                    self.register_static_eval_functions_in_expression(
                        break_hook,
                        current_function_name,
                    )?;
                }
                self.register_static_eval_functions_in_statements(body, current_function_name)?;
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
                self.register_static_eval_functions_in_expression(
                    condition,
                    current_function_name,
                )?;
                if let Some(break_hook) = break_hook {
                    self.register_static_eval_functions_in_expression(
                        break_hook,
                        current_function_name,
                    )?;
                }
                self.register_static_eval_functions_in_statements(body, current_function_name)?;
            }
            Statement::Break { .. } | Statement::Continue { .. } => {}
        }
        Ok(())
    }
}
