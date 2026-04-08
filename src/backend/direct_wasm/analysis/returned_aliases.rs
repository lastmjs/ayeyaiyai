use super::*;

#[path = "returned_aliases/enumerated_keys.rs"]
mod enumerated_keys;
#[path = "returned_aliases/identifier_returns.rs"]
mod identifier_returns;
#[path = "returned_aliases/member_aliases.rs"]
mod member_aliases;
#[path = "returned_aliases/object_literals.rs"]
mod object_literals;

pub(in crate::backend::direct_wasm) use enumerated_keys::*;
pub(in crate::backend::direct_wasm) use identifier_returns::*;
pub(in crate::backend::direct_wasm) use member_aliases::*;
pub(in crate::backend::direct_wasm) use object_literals::*;
