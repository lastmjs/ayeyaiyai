use super::*;

mod binding;
mod effects;
mod usage;

pub(in crate::backend::direct_wasm) use binding::ArgumentsValueBinding;
pub(in crate::backend::direct_wasm) use effects::{
    ArgumentsPropertyEffect, ReturnedArgumentsEffects,
};
pub(in crate::backend::direct_wasm) use usage::{
    ArgumentsIndexedPropertyState, ArgumentsSlot, ArgumentsUsage,
};
