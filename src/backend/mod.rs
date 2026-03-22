mod api;
mod direct_wasm;

pub use api::{compile_if_supported, emit_wasm, emit_wasm_with_reason};

#[cfg(test)]
#[path = "tests.rs"]
mod smoke_tests;
