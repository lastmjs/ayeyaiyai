use super::*;

mod bindings;
mod calls;
mod core;
mod emission;
mod specialization;
mod support;
mod values;

pub(in crate::backend::direct_wasm) use self::support::*;

#[cfg(test)]
mod tests;
