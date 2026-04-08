use super::super::super::*;

impl FunctionCompilerState {
    pub(in crate::backend::direct_wasm) fn push_scoped_lexical_binding(
        &mut self,
        name: &str,
        scoped_binding: String,
    ) {
        self.emission
            .lexical_scopes
            .active_scoped_lexical_bindings
            .entry(name.to_string())
            .or_default()
            .push(scoped_binding);
    }

    pub(in crate::backend::direct_wasm) fn pop_scoped_lexical_binding(&mut self, name: &str) {
        let should_remove = self
            .emission
            .lexical_scopes
            .active_scoped_lexical_bindings
            .get_mut(name)
            .is_some_and(|bindings| {
                bindings.pop();
                bindings.is_empty()
            });
        if should_remove {
            self.emission
                .lexical_scopes
                .active_scoped_lexical_bindings
                .remove(name);
        }
    }

    pub(in crate::backend::direct_wasm) fn remove_scoped_lexical_binding_occurrence(
        &mut self,
        name: &str,
        scoped_binding: &str,
    ) {
        let should_remove = self
            .emission
            .lexical_scopes
            .active_scoped_lexical_bindings
            .get_mut(name)
            .is_some_and(|bindings| {
                if bindings
                    .last()
                    .is_some_and(|binding| binding == scoped_binding)
                {
                    bindings.pop();
                } else if let Some(index) = bindings
                    .iter()
                    .rposition(|binding| binding == scoped_binding)
                {
                    bindings.remove(index);
                }
                bindings.is_empty()
            });
        if should_remove {
            self.emission
                .lexical_scopes
                .active_scoped_lexical_bindings
                .remove(name);
        }
    }
}
