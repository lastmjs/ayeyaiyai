use super::*;

mod classification;
mod function_constructors;
mod hex_arrays;
mod property_names;
mod symbols;

pub(in crate::backend::direct_wasm) use self::{
    classification::*, function_constructors::*, hex_arrays::*, property_names::*, symbols::*,
};
