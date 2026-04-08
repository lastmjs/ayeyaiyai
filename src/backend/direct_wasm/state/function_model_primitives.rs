use super::super::*;

#[derive(Clone, Copy)]
pub(in crate::backend::direct_wasm) enum PrimitiveHint {
    Default,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::direct_wasm) enum SymbolToPrimitiveHandling {
    NotHandled,
    Handled,
    AlwaysThrows,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct OrdinaryToPrimitiveStep {
    pub(in crate::backend::direct_wasm) binding: LocalFunctionBinding,
    pub(in crate::backend::direct_wasm) outcome: StaticEvalOutcome,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct OrdinaryToPrimitivePlan {
    pub(in crate::backend::direct_wasm) steps: Vec<OrdinaryToPrimitiveStep>,
}

#[derive(Clone, Copy)]
pub(in crate::backend::direct_wasm) enum OrdinaryToPrimitiveAnalysis {
    Unknown,
    Primitive(StaticValueKind),
    Throw,
    TypeError,
}
