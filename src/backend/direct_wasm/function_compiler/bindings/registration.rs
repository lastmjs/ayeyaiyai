use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn register_bindings(
        &mut self,
        statements: &[Statement],
    ) -> DirectResult<()> {
        for statement in statements {
            match statement {
                Statement::Block { body } | Statement::Labeled { body, .. } => {
                    self.register_bindings(body)?
                }
                Statement::Var { name, .. } | Statement::Let { name, .. } => {
                    if self.top_level_function && self.module.global_bindings.contains_key(name) {
                        continue;
                    }
                    if self.locals.contains_key(name) {
                        continue;
                    }
                    let next_local_index = self.next_local_index;
                    self.locals.insert(name.clone(), next_local_index);
                    self.next_local_index += 1;
                }
                Statement::If {
                    then_branch,
                    else_branch,
                    ..
                } => {
                    self.register_bindings(then_branch)?;
                    self.register_bindings(else_branch)?;
                }
                Statement::With { body, .. } => {
                    self.register_bindings(body)?;
                }
                Statement::While { body, .. } | Statement::DoWhile { body, .. } => {
                    self.register_bindings(body)?
                }
                Statement::Try {
                    body,
                    catch_binding,
                    catch_setup,
                    catch_body,
                    ..
                } => {
                    self.register_bindings(body)?;
                    if let Some(catch_binding) = catch_binding {
                        if !self.locals.contains_key(catch_binding) {
                            let next_local_index = self.next_local_index;
                            self.locals.insert(catch_binding.clone(), next_local_index);
                            self.local_kinds
                                .insert(catch_binding.clone(), StaticValueKind::Object);
                            self.next_local_index += 1;
                        }
                    }
                    self.register_bindings(catch_setup)?;
                    self.register_bindings(catch_body)?;
                }
                Statement::For {
                    init,
                    per_iteration_bindings,
                    body,
                    ..
                } => {
                    self.register_bindings(init)?;
                    for binding in per_iteration_bindings {
                        if self.locals.contains_key(binding) {
                            continue;
                        }
                        let next_local_index = self.next_local_index;
                        self.locals.insert(binding.clone(), next_local_index);
                        self.local_kinds
                            .insert(binding.clone(), StaticValueKind::Unknown);
                        self.next_local_index += 1;
                    }
                    self.register_bindings(body)?;
                }
                Statement::Switch {
                    bindings, cases, ..
                } => {
                    for binding in bindings {
                        if self.locals.contains_key(binding) {
                            continue;
                        }
                        let next_local_index = self.next_local_index;
                        self.locals.insert(binding.clone(), next_local_index);
                        self.local_kinds
                            .insert(binding.clone(), StaticValueKind::Unknown);
                        self.next_local_index += 1;
                    }
                    for case in cases {
                        self.register_bindings(&case.body)?;
                    }
                }
                Statement::Assign { .. }
                | Statement::Break { .. }
                | Statement::Continue { .. }
                | Statement::Expression(_)
                | Statement::Print { .. }
                | Statement::Return(_) => {}
                _ => {}
            }
        }

        Ok(())
    }
}
