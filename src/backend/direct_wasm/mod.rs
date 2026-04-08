use std::{
    borrow::Cow,
    cell::RefCell,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
};

use anyhow::Result;
use num_bigint::BigInt as StaticBigInt;

use crate::{
    frontend,
    ir::hir::{
        ArrayElement, BinaryOp, CallArgument, Expression, FunctionDeclaration, FunctionKind,
        ObjectEntry, Program, Statement, UnaryOp, UpdateOp,
    },
};

mod analysis;
mod constants;
mod encoding;
mod function_compiler;
mod helpers;
mod program_compiler;
mod state;
mod static_eval;

use self::{
    analysis::*,
    constants::*,
    encoding::{
        encode_code_section, encode_data_section, encode_export_section, encode_function_section,
        encode_global_section, encode_import_section, encode_memory_section, encode_type_section,
        push_i32, push_section, push_u32,
    },
    function_compiler::*,
    helpers::*,
    program_compiler::*,
    state::*,
    static_eval::*,
};

pub(in crate::backend) fn try_emit_wasm(program: &Program) -> Result<Option<Vec<u8>>> {
    let mut compiler = DirectWasmCompiler::default();
    match compiler.compile(program) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(Unsupported(_)) => Ok(None),
    }
}

pub(in crate::backend) fn emit_wasm_with_reason(
    program: &Program,
) -> std::result::Result<Vec<u8>, &'static str> {
    let mut compiler = DirectWasmCompiler::default();
    compiler
        .compile(program)
        .map_err(|Unsupported(message)| message)
}
