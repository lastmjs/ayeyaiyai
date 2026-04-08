use super::*;

#[path = "names/assigned.rs"]
mod assigned;
#[path = "names/eval_vars.rs"]
mod eval_vars;
#[path = "names/referenced.rs"]
mod referenced;

pub(in crate::backend::direct_wasm) use assigned::*;
pub(in crate::backend::direct_wasm) use eval_vars::*;
pub(in crate::backend::direct_wasm) use referenced::*;
