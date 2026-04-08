#[path = "value_binding_ops.rs"]
mod ops;
#[path = "value_binding_types.rs"]
mod types;

pub(in crate::backend::direct_wasm) use types::{
    GlobalObjectRuntimePrototypeBinding, GlobalValueService,
};
