#[path = "function_static_semantics_arrays.rs"]
mod function_static_semantics_arrays;
#[path = "function_static_semantics_core/mod.rs"]
mod function_static_semantics_core;
#[path = "function_static_semantics_objects.rs"]
mod function_static_semantics_objects;
#[path = "function_static_semantics_values.rs"]
mod function_static_semantics_values;

pub(in crate::backend::direct_wasm) use function_static_semantics_arrays::FunctionArraySemanticsState;
pub(in crate::backend::direct_wasm) use function_static_semantics_core::FunctionStaticSemanticsState;
pub(in crate::backend::direct_wasm) use function_static_semantics_objects::FunctionObjectSemanticsState;
pub(in crate::backend::direct_wasm) use function_static_semantics_values::FunctionValueSemanticsState;
