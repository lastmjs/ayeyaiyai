use super::*;

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct SimpleGeneratorStep {
    pub(in crate::backend::direct_wasm) effects: Vec<Statement>,
    pub(in crate::backend::direct_wasm) outcome: SimpleGeneratorStepOutcome,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct AsyncYieldDelegateGeneratorPlan {
    pub(in crate::backend::direct_wasm) function_name: String,
    pub(in crate::backend::direct_wasm) prefix_effects: Vec<Statement>,
    pub(in crate::backend::direct_wasm) delegate_expression: Expression,
    pub(in crate::backend::direct_wasm) completion_effects: Vec<Statement>,
    pub(in crate::backend::direct_wasm) completion_value: Expression,
    pub(in crate::backend::direct_wasm) completion_throw_value: Option<Expression>,
    pub(in crate::backend::direct_wasm) scope_bindings: Vec<String>,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) enum SimpleGeneratorStepOutcome {
    Yield(Expression),
    Throw(Expression),
}
