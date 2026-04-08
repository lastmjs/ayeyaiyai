use super::*;

#[path = "runtime/builtin_catalog.rs"]
mod builtin_catalog;
#[path = "runtime/function_metadata.rs"]
mod function_metadata;
#[path = "runtime/value_parsing.rs"]
mod value_parsing;

pub(in crate::backend::direct_wasm) use builtin_catalog::{
    bound_function_prototype_call_builtin_name, builtin_function_runtime_value,
    builtin_member_function_name, builtin_member_number_value, builtin_prototype_function_name,
    is_non_definable_global_name, parse_bound_function_prototype_call_builtin_name,
};
#[cfg(test)]
pub(in crate::backend::direct_wasm) use function_metadata::internal_function_name_hint;
pub(in crate::backend::direct_wasm) use function_metadata::{
    builtin_function_display_name, builtin_function_length, function_display_name,
    user_function_runtime_value,
};
pub(in crate::backend::direct_wasm) use value_parsing::{
    f64_to_i32, is_reserved_js_runtime_value, parse_bigint_to_i32, parse_static_bigint_literal,
    parse_string_to_i32, parse_string_to_loose_i32, parse_typeof_tag_optional,
};
