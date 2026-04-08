use super::super::super::*;

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct FunctionParameterIsolatedIndirectEvalSnapshot {
    pub(in crate::backend::direct_wasm) local_arguments_bindings:
        HashMap<String, ArgumentsValueBinding>,
    pub(in crate::backend::direct_wasm) direct_arguments_aliases: HashSet<String>,
}

impl FunctionParameterState {
    pub(in crate::backend::direct_wasm) fn capture_isolated_indirect_eval(
        &self,
    ) -> FunctionParameterIsolatedIndirectEvalSnapshot {
        FunctionParameterIsolatedIndirectEvalSnapshot {
            local_arguments_bindings: self.local_arguments_bindings.clone(),
            direct_arguments_aliases: self.direct_arguments_aliases.clone(),
        }
    }

    pub(in crate::backend::direct_wasm) fn restore_isolated_indirect_eval(
        &mut self,
        snapshot: FunctionParameterIsolatedIndirectEvalSnapshot,
    ) {
        self.local_arguments_bindings = snapshot.local_arguments_bindings;
        self.direct_arguments_aliases = snapshot.direct_arguments_aliases;
    }
}
