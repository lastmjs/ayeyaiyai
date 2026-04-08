use super::*;

#[path = "inline_summary/assertions.rs"]
mod assertions;
#[path = "inline_summary/call_frame.rs"]
mod call_frame;
#[path = "inline_summary/collection.rs"]
mod collection;
#[path = "inline_summary/substitution.rs"]
mod substitution;

pub(in crate::backend::direct_wasm) use assertions::*;
pub(in crate::backend::direct_wasm) use call_frame::*;
pub(in crate::backend::direct_wasm) use collection::*;
pub(in crate::backend::direct_wasm) use substitution::*;
