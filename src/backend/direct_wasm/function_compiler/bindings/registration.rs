use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn register_bindings(
        &mut self,
        statements: &[Statement],
    ) -> DirectResult<()> {
        for statement in statements {
            match statement {
                Statement::Declaration { body }
                | Statement::Block { body }
                | Statement::Labeled { body, .. } => self.register_bindings(body)?,
                Statement::Var { name, .. } | Statement::Let { name, .. } => {
                    if self.state.speculation.execution_context.top_level_function
                        && self.backend.global_has_binding(name)
                    {
                        continue;
                    }
                    if self.state.runtime.locals.bindings.contains_key(name) {
                        continue;
                    }
                    let next_local_index = self.state.runtime.locals.next_local_index;
                    self.state
                        .runtime
                        .locals
                        .insert(name.clone(), next_local_index);
                    self.state.runtime.locals.next_local_index += 1;
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
                        if !self
                            .state
                            .runtime
                            .locals
                            .bindings
                            .contains_key(catch_binding)
                        {
                            let next_local_index = self.state.runtime.locals.next_local_index;
                            self.state
                                .runtime
                                .locals
                                .insert(catch_binding.clone(), next_local_index);
                            self.state
                                .speculation
                                .static_semantics
                                .set_local_kind(catch_binding, StaticValueKind::Object);
                            self.state.runtime.locals.next_local_index += 1;
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
                        if self.state.runtime.locals.bindings.contains_key(binding) {
                            continue;
                        }
                        let next_local_index = self.state.runtime.locals.next_local_index;
                        self.state
                            .runtime
                            .locals
                            .insert(binding.clone(), next_local_index);
                        self.state
                            .speculation
                            .static_semantics
                            .set_local_kind(binding, StaticValueKind::Unknown);
                        self.state.runtime.locals.next_local_index += 1;
                    }
                    self.register_bindings(body)?;
                }
                Statement::Switch {
                    bindings, cases, ..
                } => {
                    for binding in bindings {
                        if self.state.runtime.locals.bindings.contains_key(binding) {
                            continue;
                        }
                        let next_local_index = self.state.runtime.locals.next_local_index;
                        self.state
                            .runtime
                            .locals
                            .insert(binding.clone(), next_local_index);
                        self.state
                            .speculation
                            .static_semantics
                            .set_local_kind(binding, StaticValueKind::Unknown);
                        self.state.runtime.locals.next_local_index += 1;
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
