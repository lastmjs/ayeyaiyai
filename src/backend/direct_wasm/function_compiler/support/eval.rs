use super::*;

#[path = "eval/helpers.rs"]
mod helpers;
#[path = "eval/local_functions.rs"]
mod local_functions;
#[path = "eval/var_names.rs"]
mod var_names;

pub(in crate::backend::direct_wasm) use helpers::{
    is_eval_local_function_declaration_statement, scoped_binding_source_name,
};
pub(in crate::backend::direct_wasm) use local_functions::{
    collect_eval_local_function_declarations, is_eval_local_function_candidate,
};
pub(in crate::backend::direct_wasm) use var_names::{
    collect_eval_var_names, collect_eval_var_names_from_statements,
    eval_statements_declare_var_arguments,
};
