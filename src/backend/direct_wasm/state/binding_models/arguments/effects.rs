use super::*;

#[derive(Clone, Default)]
pub(in crate::backend::direct_wasm) struct ReturnedArgumentsEffects {
    pub(in crate::backend::direct_wasm) callee: Option<ArgumentsPropertyEffect>,
    pub(in crate::backend::direct_wasm) length: Option<ArgumentsPropertyEffect>,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) enum ArgumentsPropertyEffect {
    Assign(Expression),
    Delete,
}
