#[derive(Clone, PartialEq, Eq, Hash)]
pub(in crate::backend::direct_wasm) enum MemberFunctionBindingTarget {
    Identifier(String),
    Prototype(String),
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub(in crate::backend::direct_wasm) enum MemberFunctionBindingProperty {
    String(String),
    Symbol(String),
    SymbolExpression(String),
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub(in crate::backend::direct_wasm) struct MemberFunctionBindingKey {
    pub(in crate::backend::direct_wasm) target: MemberFunctionBindingTarget,
    pub(in crate::backend::direct_wasm) property: MemberFunctionBindingProperty,
}
