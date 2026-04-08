use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn collect_preserved_binding_kinds_from_statement(
        &self,
        invalidated_bindings: &HashSet<String>,
        preserved_kinds: &mut HashMap<String, StaticValueKind>,
        blocked_bindings: &mut HashSet<String>,
        statement: &Statement,
    ) {
        match statement {
            Statement::Declaration { body }
            | Statement::Block { body }
            | Statement::Labeled { body, .. } => {
                for statement in body {
                    self.collect_preserved_binding_kinds_from_statement(
                        invalidated_bindings,
                        preserved_kinds,
                        blocked_bindings,
                        statement,
                    );
                }
            }
            Statement::Var { name, value } | Statement::Let { name, value, .. } => {
                self.merge_preserved_binding_kind(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    name,
                    self.infer_value_kind(value),
                );
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    value,
                );
            }
            Statement::Assign { name, value } => {
                self.merge_preserved_binding_kind(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    name,
                    self.infer_value_kind(value),
                );
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    value,
                );
            }
            Statement::AssignMember {
                object,
                property,
                value,
            } => {
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    object,
                );
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    property,
                );
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    value,
                );
            }
            Statement::Expression(expression)
            | Statement::Throw(expression)
            | Statement::Return(expression)
            | Statement::Yield { value: expression }
            | Statement::YieldDelegate { value: expression } => {
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    expression,
                );
            }
            Statement::Print { values } => {
                for value in values {
                    self.collect_preserved_binding_kinds_from_expression(
                        invalidated_bindings,
                        preserved_kinds,
                        blocked_bindings,
                        value,
                    );
                }
            }
            Statement::With { object, body } => {
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    object,
                );
                for statement in body {
                    self.collect_preserved_binding_kinds_from_statement(
                        invalidated_bindings,
                        preserved_kinds,
                        blocked_bindings,
                        statement,
                    );
                }
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    condition,
                );
                for statement in then_branch {
                    self.collect_preserved_binding_kinds_from_statement(
                        invalidated_bindings,
                        preserved_kinds,
                        blocked_bindings,
                        statement,
                    );
                }
                for statement in else_branch {
                    self.collect_preserved_binding_kinds_from_statement(
                        invalidated_bindings,
                        preserved_kinds,
                        blocked_bindings,
                        statement,
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
                    self.collect_preserved_binding_kinds_from_statement(
                        invalidated_bindings,
                        preserved_kinds,
                        blocked_bindings,
                        statement,
                    );
                }
                for statement in catch_setup {
                    self.collect_preserved_binding_kinds_from_statement(
                        invalidated_bindings,
                        preserved_kinds,
                        blocked_bindings,
                        statement,
                    );
                }
                for statement in catch_body {
                    self.collect_preserved_binding_kinds_from_statement(
                        invalidated_bindings,
                        preserved_kinds,
                        blocked_bindings,
                        statement,
                    );
                }
            }
            Statement::Switch {
                discriminant,
                cases,
                ..
            } => {
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    discriminant,
                );
                for case in cases {
                    if let Some(test) = &case.test {
                        self.collect_preserved_binding_kinds_from_expression(
                            invalidated_bindings,
                            preserved_kinds,
                            blocked_bindings,
                            test,
                        );
                    }
                    for statement in &case.body {
                        self.collect_preserved_binding_kinds_from_statement(
                            invalidated_bindings,
                            preserved_kinds,
                            blocked_bindings,
                            statement,
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
                    self.collect_preserved_binding_kinds_from_statement(
                        invalidated_bindings,
                        preserved_kinds,
                        blocked_bindings,
                        statement,
                    );
                }
                if let Some(condition) = condition {
                    self.collect_preserved_binding_kinds_from_expression(
                        invalidated_bindings,
                        preserved_kinds,
                        blocked_bindings,
                        condition,
                    );
                }
                if let Some(update) = update {
                    self.collect_preserved_binding_kinds_from_expression(
                        invalidated_bindings,
                        preserved_kinds,
                        blocked_bindings,
                        update,
                    );
                }
                if let Some(break_hook) = break_hook {
                    self.collect_preserved_binding_kinds_from_expression(
                        invalidated_bindings,
                        preserved_kinds,
                        blocked_bindings,
                        break_hook,
                    );
                }
                for statement in body {
                    self.collect_preserved_binding_kinds_from_statement(
                        invalidated_bindings,
                        preserved_kinds,
                        blocked_bindings,
                        statement,
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
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    condition,
                );
                if let Some(break_hook) = break_hook {
                    self.collect_preserved_binding_kinds_from_expression(
                        invalidated_bindings,
                        preserved_kinds,
                        blocked_bindings,
                        break_hook,
                    );
                }
                for statement in body {
                    self.collect_preserved_binding_kinds_from_statement(
                        invalidated_bindings,
                        preserved_kinds,
                        blocked_bindings,
                        statement,
                    );
                }
            }
            Statement::Break { .. } | Statement::Continue { .. } => {}
        }
    }
}
