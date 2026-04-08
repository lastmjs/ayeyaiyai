use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn rewrite_eval_scoped_captures_in_statement(
        statement: &mut Statement,
        declared_bindings: &HashSet<String>,
        eval_local_function_bindings: &HashSet<String>,
    ) {
        match statement {
            Statement::Declaration { body }
            | Statement::Block { body }
            | Statement::Labeled { body, .. } => {
                for statement in body {
                    Self::rewrite_eval_scoped_captures_in_statement(
                        statement,
                        declared_bindings,
                        eval_local_function_bindings,
                    );
                }
            }
            Statement::Var { name, value } | Statement::Let { name, value, .. } => {
                Self::rewrite_eval_scoped_binding_name(
                    name,
                    declared_bindings,
                    eval_local_function_bindings,
                );
                Self::rewrite_eval_scoped_captures_in_expression(
                    value,
                    declared_bindings,
                    eval_local_function_bindings,
                );
            }
            Statement::Assign { name, value } => {
                Self::rewrite_eval_scoped_binding_name(
                    name,
                    declared_bindings,
                    eval_local_function_bindings,
                );
                Self::rewrite_eval_scoped_captures_in_expression(
                    value,
                    declared_bindings,
                    eval_local_function_bindings,
                );
            }
            Statement::AssignMember {
                object,
                property,
                value,
            } => {
                Self::rewrite_eval_scoped_captures_in_expression(
                    object,
                    declared_bindings,
                    eval_local_function_bindings,
                );
                Self::rewrite_eval_scoped_captures_in_expression(
                    property,
                    declared_bindings,
                    eval_local_function_bindings,
                );
                Self::rewrite_eval_scoped_captures_in_expression(
                    value,
                    declared_bindings,
                    eval_local_function_bindings,
                );
            }
            Statement::Print { values } => {
                for value in values {
                    Self::rewrite_eval_scoped_captures_in_expression(
                        value,
                        declared_bindings,
                        eval_local_function_bindings,
                    );
                }
            }
            Statement::Expression(expression)
            | Statement::Throw(expression)
            | Statement::Return(expression)
            | Statement::Yield { value: expression }
            | Statement::YieldDelegate { value: expression } => {
                Self::rewrite_eval_scoped_captures_in_expression(
                    expression,
                    declared_bindings,
                    eval_local_function_bindings,
                );
            }
            Statement::With { object, body } => {
                Self::rewrite_eval_scoped_captures_in_expression(
                    object,
                    declared_bindings,
                    eval_local_function_bindings,
                );
                for statement in body {
                    Self::rewrite_eval_scoped_captures_in_statement(
                        statement,
                        declared_bindings,
                        eval_local_function_bindings,
                    );
                }
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => {
                Self::rewrite_eval_scoped_captures_in_expression(
                    condition,
                    declared_bindings,
                    eval_local_function_bindings,
                );
                for statement in then_branch {
                    Self::rewrite_eval_scoped_captures_in_statement(
                        statement,
                        declared_bindings,
                        eval_local_function_bindings,
                    );
                }
                for statement in else_branch {
                    Self::rewrite_eval_scoped_captures_in_statement(
                        statement,
                        declared_bindings,
                        eval_local_function_bindings,
                    );
                }
            }
            Statement::Try {
                catch_binding,
                body,
                catch_setup,
                catch_body,
            } => {
                if let Some(catch_binding) = catch_binding {
                    Self::rewrite_eval_scoped_binding_name(
                        catch_binding,
                        declared_bindings,
                        eval_local_function_bindings,
                    );
                }
                for statement in body {
                    Self::rewrite_eval_scoped_captures_in_statement(
                        statement,
                        declared_bindings,
                        eval_local_function_bindings,
                    );
                }
                for statement in catch_setup {
                    Self::rewrite_eval_scoped_captures_in_statement(
                        statement,
                        declared_bindings,
                        eval_local_function_bindings,
                    );
                }
                for statement in catch_body {
                    Self::rewrite_eval_scoped_captures_in_statement(
                        statement,
                        declared_bindings,
                        eval_local_function_bindings,
                    );
                }
            }
            Statement::Switch {
                discriminant,
                bindings,
                cases,
                ..
            } => {
                Self::rewrite_eval_scoped_captures_in_expression(
                    discriminant,
                    declared_bindings,
                    eval_local_function_bindings,
                );
                for binding in bindings {
                    Self::rewrite_eval_scoped_binding_name(
                        binding,
                        declared_bindings,
                        eval_local_function_bindings,
                    );
                }
                for case in cases {
                    if let Some(test) = &mut case.test {
                        Self::rewrite_eval_scoped_captures_in_expression(
                            test,
                            declared_bindings,
                            eval_local_function_bindings,
                        );
                    }
                    for statement in &mut case.body {
                        Self::rewrite_eval_scoped_captures_in_statement(
                            statement,
                            declared_bindings,
                            eval_local_function_bindings,
                        );
                    }
                }
            }
            Statement::For {
                init,
                condition,
                update,
                per_iteration_bindings,
                break_hook,
                body,
                ..
            } => {
                for statement in init {
                    Self::rewrite_eval_scoped_captures_in_statement(
                        statement,
                        declared_bindings,
                        eval_local_function_bindings,
                    );
                }
                if let Some(condition) = condition {
                    Self::rewrite_eval_scoped_captures_in_expression(
                        condition,
                        declared_bindings,
                        eval_local_function_bindings,
                    );
                }
                if let Some(update) = update {
                    Self::rewrite_eval_scoped_captures_in_expression(
                        update,
                        declared_bindings,
                        eval_local_function_bindings,
                    );
                }
                for binding in per_iteration_bindings {
                    Self::rewrite_eval_scoped_binding_name(
                        binding,
                        declared_bindings,
                        eval_local_function_bindings,
                    );
                }
                if let Some(break_hook) = break_hook {
                    Self::rewrite_eval_scoped_captures_in_expression(
                        break_hook,
                        declared_bindings,
                        eval_local_function_bindings,
                    );
                }
                for statement in body {
                    Self::rewrite_eval_scoped_captures_in_statement(
                        statement,
                        declared_bindings,
                        eval_local_function_bindings,
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
                Self::rewrite_eval_scoped_captures_in_expression(
                    condition,
                    declared_bindings,
                    eval_local_function_bindings,
                );
                if let Some(break_hook) = break_hook {
                    Self::rewrite_eval_scoped_captures_in_expression(
                        break_hook,
                        declared_bindings,
                        eval_local_function_bindings,
                    );
                }
                for statement in body {
                    Self::rewrite_eval_scoped_captures_in_statement(
                        statement,
                        declared_bindings,
                        eval_local_function_bindings,
                    );
                }
            }
            Statement::Break { .. } | Statement::Continue { .. } => {}
        }
    }
}
