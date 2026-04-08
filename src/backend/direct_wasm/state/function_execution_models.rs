#[path = "function_execution_models/emission.rs"]
mod emission;
#[path = "function_execution_models/execution_context.rs"]
mod execution_context;
#[path = "function_execution_models/lexical_scopes.rs"]
mod lexical_scopes;
#[path = "function_execution_models/speculation.rs"]
mod speculation;

use super::super::*;

pub(in crate::backend::direct_wasm) use emission::FunctionEmissionState;
pub(in crate::backend::direct_wasm) use execution_context::FunctionExecutionContextState;
pub(in crate::backend::direct_wasm) use lexical_scopes::FunctionLexicalScopeState;
pub(in crate::backend::direct_wasm) use speculation::FunctionSpeculationState;
