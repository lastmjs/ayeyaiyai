use std::{
    borrow::Cow,
    cell::RefCell,
    collections::{BTreeSet, HashMap, HashSet},
    fs,
    path::Path,
};

use anyhow::{Context, Result};
use num_bigint::BigInt as StaticBigInt;

use crate::{
    frontend, CompileOptions,
    ir::hir::{
        ArrayElement, BinaryOp, CallArgument, Expression, FunctionDeclaration, FunctionKind,
        ObjectEntry, Program, Statement, UnaryOp, UpdateOp,
    },
};

mod encoder;

use encoder::{
    encode_code_section, encode_data_section, encode_export_section, encode_function_section,
    encode_global_section, encode_import_section, encode_memory_section, encode_type_section,
    push_i32, push_section, push_u32,
};

include!("model.rs");
include!("api.rs");
include!("compiler.rs");
include!("analysis.rs");
include!("function_compiler.rs");
