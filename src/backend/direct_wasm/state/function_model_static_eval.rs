#[path = "function_model_static_eval/function_bindings.rs"]
mod function_bindings;
#[path = "function_model_static_eval/inline_effects.rs"]
mod inline_effects;
#[path = "function_model_static_eval/returned_bindings.rs"]
mod returned_bindings;
#[path = "function_model_static_eval/value_kinds.rs"]
mod value_kinds;

pub(in crate::backend::direct_wasm) use function_bindings::{
    LocalFunctionBinding, ResolvedPropertyKey,
};
pub(in crate::backend::direct_wasm) use inline_effects::{
    InlineFunctionEffect, InlineFunctionSummary, SpecializedFunctionValue, StaticEvalOutcome,
    StaticThrowValue,
};
pub(in crate::backend::direct_wasm) use returned_bindings::{
    ReturnedMemberFunctionBinding, ReturnedMemberFunctionBindingKey,
    ReturnedMemberFunctionBindingTarget, ReturnedMemberValueBinding,
};
pub(in crate::backend::direct_wasm) use value_kinds::StaticValueKind;
