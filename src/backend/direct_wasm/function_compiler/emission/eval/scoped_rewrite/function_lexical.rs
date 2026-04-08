use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn normalize_eval_scoped_bindings_to_source_names(
        &self,
        program: &mut Program,
    ) {
        DirectWasmCompiler::normalize_eval_scoped_bindings_to_source_names_impl(program);
    }

    pub(in crate::backend::direct_wasm) fn rewrite_eval_lexical_statement(
        &self,
        statement: &mut Statement,
        renamed_bindings: &HashMap<String, String>,
    ) {
        match statement {
            Statement::Declaration { body }
            | Statement::Block { body }
            | Statement::Labeled { body, .. } => {
                for statement in body {
                    self.rewrite_eval_lexical_statement(statement, renamed_bindings);
                }
            }
            Statement::Var { name, value } => {
                if let Some(renamed_name) = renamed_bindings.get(name) {
                    *name = renamed_name.clone();
                }
                self.rewrite_eval_lexical_expression(value, renamed_bindings);
            }
            Statement::Let { name, value, .. } => {
                if let Some(renamed_name) = renamed_bindings.get(name) {
                    *name = renamed_name.clone();
                }
                self.rewrite_eval_lexical_expression(value, renamed_bindings);
            }
            Statement::Assign { name, value } => {
                if let Some(renamed_name) = renamed_bindings.get(name) {
                    *name = renamed_name.clone();
                }
                self.rewrite_eval_lexical_expression(value, renamed_bindings);
            }
            Statement::AssignMember {
                object,
                property,
                value,
            } => {
                self.rewrite_eval_lexical_expression(object, renamed_bindings);
                self.rewrite_eval_lexical_expression(property, renamed_bindings);
                self.rewrite_eval_lexical_expression(value, renamed_bindings);
            }
            Statement::Print { values } => {
                for value in values {
                    self.rewrite_eval_lexical_expression(value, renamed_bindings);
                }
            }
            Statement::Expression(expression)
            | Statement::Throw(expression)
            | Statement::Return(expression)
            | Statement::Yield { value: expression }
            | Statement::YieldDelegate { value: expression } => {
                self.rewrite_eval_lexical_expression(expression, renamed_bindings);
            }
            Statement::With { object, body } => {
                self.rewrite_eval_lexical_expression(object, renamed_bindings);
                for statement in body {
                    self.rewrite_eval_lexical_statement(statement, renamed_bindings);
                }
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.rewrite_eval_lexical_expression(condition, renamed_bindings);
                for statement in then_branch {
                    self.rewrite_eval_lexical_statement(statement, renamed_bindings);
                }
                for statement in else_branch {
                    self.rewrite_eval_lexical_statement(statement, renamed_bindings);
                }
            }
            Statement::Try {
                body,
                catch_setup,
                catch_body,
                ..
            } => {
                for statement in body {
                    self.rewrite_eval_lexical_statement(statement, renamed_bindings);
                }
                for statement in catch_setup {
                    self.rewrite_eval_lexical_statement(statement, renamed_bindings);
                }
                for statement in catch_body {
                    self.rewrite_eval_lexical_statement(statement, renamed_bindings);
                }
            }
            Statement::Switch {
                discriminant,
                cases,
                ..
            } => {
                self.rewrite_eval_lexical_expression(discriminant, renamed_bindings);
                for case in cases {
                    if let Some(test) = &mut case.test {
                        self.rewrite_eval_lexical_expression(test, renamed_bindings);
                    }
                    for statement in &mut case.body {
                        self.rewrite_eval_lexical_statement(statement, renamed_bindings);
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
                    self.rewrite_eval_lexical_statement(statement, renamed_bindings);
                }
                if let Some(condition) = condition {
                    self.rewrite_eval_lexical_expression(condition, renamed_bindings);
                }
                if let Some(update) = update {
                    self.rewrite_eval_lexical_expression(update, renamed_bindings);
                }
                if let Some(break_hook) = break_hook {
                    self.rewrite_eval_lexical_expression(break_hook, renamed_bindings);
                }
                for statement in body {
                    self.rewrite_eval_lexical_statement(statement, renamed_bindings);
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
                self.rewrite_eval_lexical_expression(condition, renamed_bindings);
                if let Some(break_hook) = break_hook {
                    self.rewrite_eval_lexical_expression(break_hook, renamed_bindings);
                }
                for statement in body {
                    self.rewrite_eval_lexical_statement(statement, renamed_bindings);
                }
            }
            Statement::Break { .. } | Statement::Continue { .. } => {}
        }
    }

    pub(in crate::backend::direct_wasm) fn rewrite_eval_lexical_expression(
        &self,
        expression: &mut Expression,
        renamed_bindings: &HashMap<String, String>,
    ) {
        match expression {
            Expression::Identifier(name) | Expression::Update { name, .. } => {
                if let Some(renamed_name) = renamed_bindings.get(name) {
                    *name = renamed_name.clone();
                }
            }
            Expression::Array(elements) => {
                for element in elements {
                    match element {
                        crate::ir::hir::ArrayElement::Expression(expression)
                        | crate::ir::hir::ArrayElement::Spread(expression) => {
                            self.rewrite_eval_lexical_expression(expression, renamed_bindings);
                        }
                    }
                }
            }
            Expression::Object(entries) => {
                for entry in entries {
                    match entry {
                        ObjectEntry::Data { key, value } => {
                            self.rewrite_eval_lexical_expression(key, renamed_bindings);
                            self.rewrite_eval_lexical_expression(value, renamed_bindings);
                        }
                        ObjectEntry::Getter { key, getter } => {
                            self.rewrite_eval_lexical_expression(key, renamed_bindings);
                            self.rewrite_eval_lexical_expression(getter, renamed_bindings);
                        }
                        ObjectEntry::Setter { key, setter } => {
                            self.rewrite_eval_lexical_expression(key, renamed_bindings);
                            self.rewrite_eval_lexical_expression(setter, renamed_bindings);
                        }
                        ObjectEntry::Spread(expression) => {
                            self.rewrite_eval_lexical_expression(expression, renamed_bindings);
                        }
                    }
                }
            }
            Expression::Member { object, property } => {
                self.rewrite_eval_lexical_expression(object, renamed_bindings);
                self.rewrite_eval_lexical_expression(property, renamed_bindings);
            }
            Expression::SuperMember { property } => {
                self.rewrite_eval_lexical_expression(property, renamed_bindings);
            }
            Expression::Assign { name, value } => {
                if let Some(renamed_name) = renamed_bindings.get(name) {
                    *name = renamed_name.clone();
                }
                self.rewrite_eval_lexical_expression(value, renamed_bindings);
            }
            Expression::AssignMember {
                object,
                property,
                value,
            } => {
                self.rewrite_eval_lexical_expression(object, renamed_bindings);
                self.rewrite_eval_lexical_expression(property, renamed_bindings);
                self.rewrite_eval_lexical_expression(value, renamed_bindings);
            }
            Expression::AssignSuperMember { property, value } => {
                self.rewrite_eval_lexical_expression(property, renamed_bindings);
                self.rewrite_eval_lexical_expression(value, renamed_bindings);
            }
            Expression::Await(expression)
            | Expression::EnumerateKeys(expression)
            | Expression::GetIterator(expression)
            | Expression::IteratorClose(expression)
            | Expression::Unary { expression, .. } => {
                self.rewrite_eval_lexical_expression(expression, renamed_bindings);
            }
            Expression::Binary { left, right, .. } => {
                self.rewrite_eval_lexical_expression(left, renamed_bindings);
                self.rewrite_eval_lexical_expression(right, renamed_bindings);
            }
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                self.rewrite_eval_lexical_expression(condition, renamed_bindings);
                self.rewrite_eval_lexical_expression(then_expression, renamed_bindings);
                self.rewrite_eval_lexical_expression(else_expression, renamed_bindings);
            }
            Expression::Sequence(expressions) => {
                for expression in expressions {
                    self.rewrite_eval_lexical_expression(expression, renamed_bindings);
                }
            }
            Expression::Call { callee, arguments }
            | Expression::SuperCall { callee, arguments }
            | Expression::New { callee, arguments } => {
                self.rewrite_eval_lexical_expression(callee, renamed_bindings);
                for argument in arguments {
                    match argument {
                        CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                            self.rewrite_eval_lexical_expression(expression, renamed_bindings);
                        }
                    }
                }
            }
            Expression::Number(_)
            | Expression::BigInt(_)
            | Expression::String(_)
            | Expression::Bool(_)
            | Expression::Null
            | Expression::Undefined
            | Expression::NewTarget
            | Expression::This
            | Expression::Sent => {}
        }
    }
}
