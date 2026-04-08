use super::*;

impl DirectWasmCompiler {
    fn register_global_bindings_in_statements(
        &mut self,
        statements: &[Statement],
        next_global_index: &mut u32,
    ) {
        for statement in statements {
            match statement {
                Statement::Declaration { body }
                | Statement::Block { body }
                | Statement::Labeled { body, .. } => {
                    self.register_global_bindings_in_statements(body, next_global_index);
                }
                Statement::Var { name, value } => {
                    self.ensure_global_binding_index(name, next_global_index);
                    if self.global_binding_kind(name).is_none() {
                        self.set_global_binding_kind(name, infer_global_expression_kind(value));
                    }
                    self.upsert_global_data_property_descriptor(
                        name,
                        self.materialize_global_expression(value),
                        Some(true),
                        true,
                        false,
                    );
                    self.update_static_global_assignment_metadata(name, value);
                }
                Statement::Let { name, value, .. } => {
                    self.ensure_global_binding_index(name, next_global_index);
                    self.mark_global_lexical_binding(name);
                    if self.global_binding_kind(name).is_none() {
                        self.set_global_binding_kind(name, infer_global_expression_kind(value));
                    }
                    self.update_static_global_assignment_metadata(name, value);
                }
                Statement::Assign { name, value } => {
                    if self.global_has_binding(name) {
                        self.update_static_global_assignment_metadata(name, value);
                    }
                }
                Statement::AssignMember {
                    object,
                    property,
                    value,
                } => {
                    self.update_global_member_assignment_metadata(object, property, value);
                }
                Statement::Expression(expression) => {
                    self.update_global_expression_metadata(expression);
                }
                _ => {}
            }
        }
    }

    fn enclosing_self_binding_capture_source_name(
        functions: &[FunctionDeclaration],
        function_index: usize,
        source_name: &str,
    ) -> Option<String> {
        let function = functions.get(function_index)?;
        functions
            .iter()
            .enumerate()
            .skip(function_index + 1)
            .find(|(_, candidate)| {
                if candidate.self_binding.as_deref() != Some(source_name) {
                    return false;
                }
                if collect_referenced_binding_names_from_statements(&candidate.body)
                    .contains(&function.name)
                {
                    return true;
                }
                candidate.params.iter().any(|parameter| {
                    parameter.default.as_ref().is_some_and(|default| {
                        let mut referenced = HashSet::new();
                        collect_referenced_binding_names_from_expression(default, &mut referenced);
                        referenced.contains(&function.name)
                    })
                })
            })
            .map(|(_, candidate)| candidate.name.clone())
    }

    pub(in crate::backend::direct_wasm) fn register_global_bindings(
        &mut self,
        statements: &[Statement],
    ) {
        let mut next_global_index = self.next_allocated_global_index();
        self.register_global_bindings_in_statements(statements, &mut next_global_index);
    }

    pub(in crate::backend::direct_wasm) fn register_global_function_bindings(
        &mut self,
        functions: &[FunctionDeclaration],
    ) {
        let mut next_global_index = self.next_allocated_global_index();

        for function in functions {
            if !function.register_global {
                continue;
            }

            self.ensure_global_binding_index(&function.name, &mut next_global_index);
            self.set_global_user_function_reference(&function.name);
            self.upsert_global_data_property_descriptor(
                &function.name,
                Expression::Identifier(function.name.clone()),
                Some(true),
                true,
                false,
            );
        }
    }

    pub(in crate::backend::direct_wasm) fn register_user_function_capture_bindings(
        &mut self,
        functions: &[FunctionDeclaration],
    ) {
        self.clear_user_function_capture_bindings();

        for (function_index, function) in functions.iter().enumerate() {
            let scope_bindings = collect_function_constructor_local_bindings(function)
                .into_iter()
                .map(|name| {
                    scoped_binding_source_name(&name)
                        .unwrap_or(&name)
                        .to_string()
                })
                .collect::<HashSet<_>>();
            let referenced = collect_referenced_binding_names_from_statements(&function.body);
            let mut captures = HashMap::new();

            for name in referenced {
                let is_scoped_binding = scoped_binding_source_name(&name).is_some();
                let source_name = scoped_binding_source_name(&name)
                    .unwrap_or(&name)
                    .to_string();
                let capture_source_name = Self::enclosing_self_binding_capture_source_name(
                    functions,
                    function_index,
                    &source_name,
                )
                .unwrap_or_else(|| source_name.clone());
                if scope_bindings.contains(&source_name)
                    || (!is_scoped_binding && self.contains_user_function(&source_name))
                    || self.global_has_binding(&source_name)
                    || self.global_has_lexical_binding(&source_name)
                    || self.global_function_binding(&source_name).is_some()
                    || self.global_has_implicit_binding(&source_name)
                    || is_builtin_like_capture_identifier(&source_name)
                {
                    continue;
                }

                let hidden_name = format!(
                    "__ayy_capture_binding__{}__{}",
                    function.name, capture_source_name
                );
                self.ensure_implicit_global_binding(&hidden_name);
                captures.entry(capture_source_name).or_insert(hidden_name);
            }

            if !captures.is_empty() {
                self.set_user_function_capture_bindings(&function.name, captures);
            }
        }
    }

    pub(in crate::backend::direct_wasm) fn reserve_global_array_runtime_state_bindings(
        &mut self,
        program: &Program,
    ) {
        let global_array_names = self
            .global_array_binding_entries()
            .into_iter()
            .map(|(name, _)| name)
            .collect::<HashSet<_>>();
        for function in &program.functions {
            let local_bindings = collect_function_constructor_local_bindings(function)
                .into_iter()
                .map(|name| {
                    scoped_binding_source_name(&name)
                        .unwrap_or(&name)
                        .to_string()
                })
                .collect::<HashSet<_>>();
            let mut referenced = collect_referenced_binding_names_from_statements(&function.body);
            for parameter in &function.params {
                if let Some(default) = &parameter.default {
                    collect_referenced_binding_names_from_expression(default, &mut referenced);
                }
            }
            for name in referenced {
                let source_name = scoped_binding_source_name(&name).unwrap_or(&name);
                if local_bindings.contains(source_name) {
                    continue;
                }
                if global_array_names.contains(source_name) {
                    self.mark_global_array_with_runtime_state(source_name);
                }
            }
        }
    }

    pub(in crate::backend::direct_wasm) fn reserve_function_constructor_implicit_global_bindings(
        &mut self,
        program: &Program,
    ) -> DirectResult<()> {
        let mut names = BTreeSet::new();
        for function in &program.functions {
            if !function.name.starts_with("__ayy_function_ctor_") {
                continue;
            }
            let scope = collect_function_constructor_local_bindings(function);
            collect_implicit_globals_from_statements(
                &function.body,
                function.strict,
                &scope,
                &mut names,
            )?;
        }

        for name in names {
            if self.global_has_binding(&name) || self.global_has_implicit_binding(&name) {
                continue;
            }
            self.create_implicit_global_binding(&name);
        }

        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn ensure_implicit_global_binding(
        &mut self,
        name: &str,
    ) -> ImplicitGlobalBinding {
        self.create_implicit_global_binding(name)
    }

    pub(in crate::backend::direct_wasm) fn next_allocated_global_index(&self) -> u32 {
        self.next_available_global_index()
    }

    pub(in crate::backend::direct_wasm) fn reserve_global_runtime_prototype_binding_globals(
        &mut self,
    ) {
        let mut names = self.runtime_prototype_binding_names();
        names.sort();
        let mut next_global_index = self.next_allocated_global_index();
        for name in names {
            self.set_runtime_prototype_binding_global_index(&name, next_global_index);
            next_global_index += 1;
        }
    }
}
