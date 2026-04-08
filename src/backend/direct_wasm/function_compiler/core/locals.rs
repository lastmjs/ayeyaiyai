use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn allocate_named_hidden_local(
        &mut self,
        prefix: &str,
        kind: StaticValueKind,
    ) -> String {
        let name = format!(
            "__ayy_{prefix}_{}",
            self.state.runtime.locals.next_local_index
        );
        let next_local_index = self.state.runtime.locals.next_local_index;
        self.state
            .runtime
            .locals
            .insert(name.clone(), next_local_index);
        self.state
            .speculation
            .static_semantics
            .set_local_kind(&name, kind);
        self.state.runtime.locals.next_local_index += 1;
        name
    }
}
