use super::*;

mod bindings;
mod descriptors;
mod members;

pub(in crate::backend::direct_wasm) use bindings::{ObjectValueBinding, ProxyValueBinding};
pub(in crate::backend::direct_wasm) use descriptors::{
    GlobalPropertyDescriptorState, PropertyDescriptorBinding, PropertyDescriptorDefinition,
    StringConcatFragment,
};
pub(in crate::backend::direct_wasm) use members::{
    MemberFunctionBindingKey, MemberFunctionBindingProperty, MemberFunctionBindingTarget,
};
