use super::*;

mod bindings;
mod eval;
mod globals;
mod names;
mod references;
mod runtime;

pub(in crate::backend::direct_wasm) use self::{
    bindings::*, eval::*, globals::*, names::*, references::*, runtime::*,
};
