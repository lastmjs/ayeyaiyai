use super::super::super::*;
use super::function_bindings::LocalFunctionBinding;

#[derive(Clone)]
pub(in crate::backend::direct_wasm) enum InlineFunctionEffect {
    Assign {
        name: String,
        value: Expression,
    },
    Update {
        name: String,
        op: UpdateOp,
        prefix: bool,
    },
    Expression(Expression),
}

#[derive(Clone, Default)]
pub(in crate::backend::direct_wasm) struct InlineFunctionSummary {
    pub(in crate::backend::direct_wasm) effects: Vec<InlineFunctionEffect>,
    pub(in crate::backend::direct_wasm) return_value: Option<Expression>,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct SpecializedFunctionValue {
    pub(in crate::backend::direct_wasm) binding: LocalFunctionBinding,
    pub(in crate::backend::direct_wasm) summary: InlineFunctionSummary,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) enum StaticThrowValue {
    Value(Expression),
    NamedError(&'static str),
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) enum StaticEvalOutcome {
    Value(Expression),
    Throw(StaticThrowValue),
}
