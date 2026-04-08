use super::*;

#[path = "helpers/eval_namespace.rs"]
mod eval_namespace;
#[path = "helpers/eval_rewrite.rs"]
mod eval_rewrite;
#[path = "helpers/object_values.rs"]
mod object_values;
#[path = "helpers/realm_names.rs"]
mod realm_names;

pub(in crate::backend::direct_wasm) use self::{
    eval_rewrite::namespace_eval_program_internal_function_names,
    object_values::empty_object_value_binding,
    realm_names::{
        parse_test262_realm_eval_builtin, parse_test262_realm_global_identifier,
        parse_test262_realm_identifier, test262_realm_eval_builtin_name,
        test262_realm_global_identifier, test262_realm_identifier,
    },
};
