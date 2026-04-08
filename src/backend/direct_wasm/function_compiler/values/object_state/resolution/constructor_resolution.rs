use super::*;

mod constructor_bindings;
mod new_target_rewrite;
mod snapshot_analysis;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) const STATIC_NEW_THIS_BINDING: &'static str =
        "__ayy_static_new_this";
    pub(in crate::backend::direct_wasm) const STATIC_NEW_THIS_INITIALIZED_BINDING: &'static str =
        "__ayy_static_new_this_initialized";
}
