#[path = "function_model_captures.rs"]
mod captures;
#[path = "function_model_primitives.rs"]
mod primitives;
#[path = "function_model_static_eval.rs"]
mod static_eval;
#[path = "function_model_user_function.rs"]
mod user_function;

pub(in crate::backend::direct_wasm) use captures::{
    BoundUserFunctionCallSnapshot, PreparedBoundCaptureBinding, PreparedCaptureBinding,
};
pub(in crate::backend::direct_wasm) use primitives::{
    OrdinaryToPrimitiveAnalysis, OrdinaryToPrimitivePlan, OrdinaryToPrimitiveStep, PrimitiveHint,
    SymbolToPrimitiveHandling,
};
pub(in crate::backend::direct_wasm) use static_eval::{
    InlineFunctionEffect, InlineFunctionSummary, LocalFunctionBinding, ResolvedPropertyKey,
    ReturnedMemberFunctionBinding, ReturnedMemberFunctionBindingKey,
    ReturnedMemberFunctionBindingTarget, ReturnedMemberValueBinding, SpecializedFunctionValue,
    StaticEvalOutcome, StaticThrowValue, StaticValueKind,
};
pub(in crate::backend::direct_wasm) use user_function::UserFunction;
