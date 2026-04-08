use super::*;

mod analysis;
mod catalog;
mod compiler_services;
mod state_services;
mod types;

pub(in crate::backend::direct_wasm) use analysis::{
    PreparedFunctionParameterBindings, UserFunctionAnalysisRegistry, UserFunctionParameterAnalysis,
};
pub(in crate::backend::direct_wasm) use catalog::UserFunctionCatalog;
pub(in crate::backend::direct_wasm) use types::UserTypeRegistry;

#[derive(Default)]
pub(in crate::backend::direct_wasm) struct FunctionRegistryState {
    pub(in crate::backend::direct_wasm) types: UserTypeRegistry,
    pub(in crate::backend::direct_wasm) catalog: UserFunctionCatalog,
    pub(in crate::backend::direct_wasm) analysis: UserFunctionAnalysisRegistry,
}

impl FunctionRegistryState {}
