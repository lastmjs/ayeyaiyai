use super::super::*;

impl FunctionCompilerState {
    pub(in crate::backend::direct_wasm) fn push_active_eval_lexical_scope(
        &mut self,
        names: Vec<String>,
    ) {
        let mut pushed = Vec::new();
        let mut seen = HashSet::new();
        for name in names {
            let source_name = scoped_binding_source_name(&name)
                .unwrap_or(&name)
                .to_string();
            if !seen.insert(source_name.clone()) {
                continue;
            }
            *self
                .emission
                .lexical_scopes
                .active_eval_lexical_binding_counts
                .entry(source_name.clone())
                .or_insert(0) += 1;
            let active_binding =
                (name != source_name && self.runtime.locals.contains_key(&name)).then_some(name);
            if let Some(active_binding) = active_binding.as_ref() {
                self.push_scoped_lexical_binding(&source_name, active_binding.clone());
            }
            pushed.push((source_name, active_binding));
        }
        self.emission
            .lexical_scopes
            .active_eval_lexical_scopes
            .push(pushed);
    }

    pub(in crate::backend::direct_wasm) fn pop_active_eval_lexical_scope(&mut self) {
        let Some(names) = self
            .emission
            .lexical_scopes
            .active_eval_lexical_scopes
            .pop()
        else {
            return;
        };
        for (name, active_binding) in names {
            let Some(count) = self
                .emission
                .lexical_scopes
                .active_eval_lexical_binding_counts
                .get_mut(&name)
            else {
                continue;
            };
            *count -= 1;
            if *count == 0 {
                self.emission
                    .lexical_scopes
                    .active_eval_lexical_binding_counts
                    .remove(&name);
            }
            if let Some(active_binding) = active_binding {
                self.remove_scoped_lexical_binding_occurrence(&name, &active_binding);
            }
        }
    }
}
