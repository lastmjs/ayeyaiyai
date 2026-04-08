use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn hidden_implicit_global_binding(
        &self,
        hidden_name: &str,
    ) -> Option<ImplicitGlobalBinding> {
        self.backend.implicit_global_binding(hidden_name)
    }

    pub(in crate::backend::direct_wasm) fn resolve_global_binding_index(
        &self,
        name: &str,
    ) -> Option<u32> {
        self.backend.resolve_global_binding_index(name)
    }

    pub(in crate::backend::direct_wasm) fn resolve_eval_local_function_hidden_name(
        &self,
        name: &str,
    ) -> Option<String> {
        let current_function_name = self.current_function_name()?;
        let bindings = self.eval_local_function_bindings(current_function_name)?;
        if let Some(hidden_name) = bindings.get(name) {
            return Some(hidden_name.clone());
        }

        let renamed_prefix = format!("__ayy_scope${name}$");
        let mut resolved: Option<(u32, String)> = None;
        for (candidate_name, hidden_name) in bindings {
            if !candidate_name.starts_with(&renamed_prefix) {
                continue;
            }
            let Some((_, scope_id)) = candidate_name.rsplit_once('$') else {
                continue;
            };
            let Ok(scope_id) = scope_id.parse::<u32>() else {
                continue;
            };
            if resolved
                .as_ref()
                .is_none_or(|(best_scope_id, _)| scope_id > *best_scope_id)
            {
                resolved = Some((scope_id, hidden_name.clone()));
            }
        }

        resolved.map(|(_, hidden_name)| hidden_name)
    }

    pub(in crate::backend::direct_wasm) fn resolve_user_function_capture_hidden_name(
        &self,
        name: &str,
    ) -> Option<String> {
        let current_function_name = self.current_function_name()?;
        let bindings = self.user_function_capture_bindings(current_function_name)?;
        if let Some(hidden_name) = bindings.get(name) {
            return Some(hidden_name.clone());
        }

        let source_name = scoped_binding_source_name(name);
        if let Some(source_name) = source_name
            && let Some(hidden_name) = bindings.get(source_name)
        {
            return Some(hidden_name.clone());
        }

        bindings.iter().find_map(|(capture_name, hidden_name)| {
            self.resolve_registered_function_declaration(capture_name)
                .and_then(|function| function.self_binding.as_deref())
                .filter(|self_binding| {
                    *self_binding == name
                        || source_name.is_some_and(|source_name| *self_binding == source_name)
                })
                .map(|_| hidden_name.clone())
        })
    }

    pub(in crate::backend::direct_wasm) fn resolve_current_local_binding(
        &self,
        name: &str,
    ) -> Option<(String, u32)> {
        fn resolve_current_local_binding_exact(
            locals: &HashMap<String, u32>,
            active_scoped_lexical_bindings: &HashMap<String, Vec<String>>,
            name: &str,
        ) -> Option<(String, u32)> {
            if let Some(active_name) = active_scoped_lexical_bindings
                .get(name)
                .and_then(|bindings| bindings.last())
                .cloned()
            {
                if let Some(local_index) = locals.get(&active_name).copied() {
                    return Some((active_name, local_index));
                }
            }

            if let Some(local_index) = locals.get(name).copied() {
                return Some((name.to_string(), local_index));
            }

            let mut scoped_matches = locals.iter().filter_map(|(binding_name, &local_index)| {
                (scoped_binding_source_name(binding_name) == Some(name))
                    .then(|| (binding_name.clone(), local_index))
            });
            let scoped_match = scoped_matches.next()?;
            scoped_matches.next().is_none().then_some(scoped_match)
        }

        if let Some(resolved) = resolve_current_local_binding_exact(
            &self.state.runtime.locals.bindings,
            &self
                .state
                .emission
                .lexical_scopes
                .active_scoped_lexical_bindings,
            name,
        ) {
            return Some(resolved);
        }
        if let Some(source_name) = scoped_binding_source_name(name) {
            return resolve_current_local_binding_exact(
                &self.state.runtime.locals.bindings,
                &self
                    .state
                    .emission
                    .lexical_scopes
                    .active_scoped_lexical_bindings,
                source_name,
            );
        }

        None
    }

    pub(in crate::backend::direct_wasm) fn emit_eval_lexical_binding_read(
        &mut self,
        name: &str,
    ) -> DirectResult<bool> {
        let Some(initialized_local) = self
            .state
            .speculation
            .static_semantics
            .eval_lexical_initialized_locals
            .get(name)
            .copied()
        else {
            return Ok(false);
        };
        let local_index = self
            .state
            .runtime
            .locals
            .get(name)
            .copied()
            .expect("tracked eval lexical binding must have a local slot");
        self.push_local_get(initialized_local);
        self.state.emission.output.instructions.push(0x04);
        self.state.emission.output.instructions.push(I32_TYPE);
        self.push_control_frame();
        self.push_local_get(local_index);
        self.state.emission.output.instructions.push(0x05);
        self.emit_named_error_throw("ReferenceError")?;
        self.state.emission.output.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(true)
    }
}
