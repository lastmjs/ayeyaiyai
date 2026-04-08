#[path = "function/execution_context.rs"]
mod execution_context;
#[path = "function/lexical_scopes.rs"]
mod lexical_scopes;
#[path = "function/parameters.rs"]
mod parameters;
#[path = "function/runtime.rs"]
mod runtime;
#[path = "function/static_bindings.rs"]
mod static_bindings;

pub(in crate::backend::direct_wasm) use execution_context::{
    FunctionExecutionContextSnapshot, UserFunctionExecutionContextSnapshot,
};
pub(in crate::backend::direct_wasm) use lexical_scopes::FunctionLexicalScopeSnapshot;
pub(in crate::backend::direct_wasm) use parameters::FunctionParameterIsolatedIndirectEvalSnapshot;
pub(in crate::backend::direct_wasm) use runtime::FunctionRuntimeIsolatedIndirectEvalSnapshot;
pub(in crate::backend::direct_wasm) use static_bindings::{
    FunctionStaticBindingMetadataSnapshot, FunctionStaticBindingMetadataTransaction,
    LocalStaticBindingSnapshot, LocalStaticBindingState,
};
