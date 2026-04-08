use super::*;

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct PreparedFunctionCompilerInputs {
    pub(in crate::backend::direct_wasm) shared_program: PreparedSharedProgramContext,
    pub(in crate::backend::direct_wasm) assigned_nonlocal_binding_results:
        Rc<HashMap<String, HashMap<String, Expression>>>,
}

impl PreparedFunctionCompilerInputs {
    pub(in crate::backend::direct_wasm) fn shared_program_context(
        &self,
    ) -> PreparedSharedProgramContext {
        self.shared_program.clone()
    }

    pub(in crate::backend::direct_wasm) fn assigned_nonlocal_binding_results_snapshot(
        &self,
    ) -> Rc<HashMap<String, HashMap<String, Expression>>> {
        self.assigned_nonlocal_binding_results.clone()
    }
}
