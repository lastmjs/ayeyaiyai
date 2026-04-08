use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn collect_parameter_get_iterator_names_from_control_flow_statement(
        statement: &Statement,
        param_names: &HashSet<String>,
        consumed_names: &mut HashSet<String>,
    ) {
        match statement {
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => {
                Self::collect_parameter_get_iterator_names_from_expression(
                    condition,
                    param_names,
                    consumed_names,
                );
                Self::collect_parameter_get_iterator_names_from_statements(
                    then_branch,
                    param_names,
                    consumed_names,
                );
                Self::collect_parameter_get_iterator_names_from_statements(
                    else_branch,
                    param_names,
                    consumed_names,
                );
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
                Self::collect_parameter_get_iterator_names_from_expression(
                    condition,
                    param_names,
                    consumed_names,
                );
                if let Some(break_hook) = break_hook {
                    Self::collect_parameter_get_iterator_names_from_expression(
                        break_hook,
                        param_names,
                        consumed_names,
                    );
                }
                Self::collect_parameter_get_iterator_names_from_statements(
                    body,
                    param_names,
                    consumed_names,
                );
            }
            Statement::For {
                init,
                condition,
                update,
                break_hook,
                body,
                ..
            } => {
                Self::collect_parameter_get_iterator_names_from_statements(
                    init,
                    param_names,
                    consumed_names,
                );
                if let Some(condition) = condition {
                    Self::collect_parameter_get_iterator_names_from_expression(
                        condition,
                        param_names,
                        consumed_names,
                    );
                }
                if let Some(update) = update {
                    Self::collect_parameter_get_iterator_names_from_expression(
                        update,
                        param_names,
                        consumed_names,
                    );
                }
                if let Some(break_hook) = break_hook {
                    Self::collect_parameter_get_iterator_names_from_expression(
                        break_hook,
                        param_names,
                        consumed_names,
                    );
                }
                Self::collect_parameter_get_iterator_names_from_statements(
                    body,
                    param_names,
                    consumed_names,
                );
            }
            Statement::Try {
                body,
                catch_setup,
                catch_body,
                ..
            } => {
                Self::collect_parameter_get_iterator_names_from_statements(
                    body,
                    param_names,
                    consumed_names,
                );
                Self::collect_parameter_get_iterator_names_from_statements(
                    catch_setup,
                    param_names,
                    consumed_names,
                );
                Self::collect_parameter_get_iterator_names_from_statements(
                    catch_body,
                    param_names,
                    consumed_names,
                );
            }
            Statement::Switch {
                discriminant,
                cases,
                ..
            } => {
                Self::collect_parameter_get_iterator_names_from_expression(
                    discriminant,
                    param_names,
                    consumed_names,
                );
                for case in cases {
                    if let Some(test) = &case.test {
                        Self::collect_parameter_get_iterator_names_from_expression(
                            test,
                            param_names,
                            consumed_names,
                        );
                    }
                    Self::collect_parameter_get_iterator_names_from_statements(
                        &case.body,
                        param_names,
                        consumed_names,
                    );
                }
            }
            Statement::Break { .. } | Statement::Continue { .. } => {}
            _ => {}
        }
    }
}
