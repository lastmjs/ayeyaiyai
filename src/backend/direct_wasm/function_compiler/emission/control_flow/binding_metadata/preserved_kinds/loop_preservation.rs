use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn preserved_binding_kinds_for_loop(
        &self,
        invalidated_bindings: &HashSet<String>,
        condition: &Expression,
        break_hook: Option<&Expression>,
        body: &[Statement],
        update: Option<&Expression>,
    ) -> HashMap<String, StaticValueKind> {
        let mut preserved_kinds = HashMap::new();
        for name in invalidated_bindings {
            if let Some(kind) = self.current_binding_kind_for_preservation(name) {
                preserved_kinds.insert(name.clone(), kind);
            }
        }
        let mut blocked_bindings = HashSet::new();
        self.collect_preserved_binding_kinds_from_expression(
            invalidated_bindings,
            &mut preserved_kinds,
            &mut blocked_bindings,
            condition,
        );
        if let Some(update) = update {
            self.collect_preserved_binding_kinds_from_expression(
                invalidated_bindings,
                &mut preserved_kinds,
                &mut blocked_bindings,
                update,
            );
        }
        if let Some(break_hook) = break_hook {
            self.collect_preserved_binding_kinds_from_expression(
                invalidated_bindings,
                &mut preserved_kinds,
                &mut blocked_bindings,
                break_hook,
            );
        }
        for statement in body {
            self.collect_preserved_binding_kinds_from_statement(
                invalidated_bindings,
                &mut preserved_kinds,
                &mut blocked_bindings,
                statement,
            );
        }
        preserved_kinds
    }
}
