use super::*;

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct PreparedStartFunction {
    pub(in crate::backend::direct_wasm) statements: Vec<Statement>,
    pub(in crate::backend::direct_wasm) entry_state: PreparedFunctionEntryState,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct PreparedUserFunctionAnalysis {
    pub(in crate::backend::direct_wasm) assigned_nonlocal_bindings: HashSet<String>,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct PreparedFunctionMetadata {
    pub(in crate::backend::direct_wasm) name: String,
    pub(in crate::backend::direct_wasm) declaration: FunctionDeclaration,
    pub(in crate::backend::direct_wasm) user_function: UserFunction,
}

impl PreparedFunctionMetadata {
    pub(in crate::backend::direct_wasm) fn body(&self) -> &[Statement] {
        &self.declaration.body
    }
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct PreparedUserFunctionCompilation {
    pub(in crate::backend::direct_wasm) metadata: PreparedFunctionMetadata,
    pub(in crate::backend::direct_wasm) analysis: PreparedUserFunctionAnalysis,
    pub(in crate::backend::direct_wasm) entry_state: PreparedFunctionEntryState,
}
