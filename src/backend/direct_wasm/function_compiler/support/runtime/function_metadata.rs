use super::*;

pub(in crate::backend::direct_wasm) fn user_function_runtime_value(
    user_function: &UserFunction,
) -> i32 {
    let offset = user_function
        .function_index
        .saturating_sub(USER_FUNCTION_BASE_INDEX);
    debug_assert!(offset < JS_USER_FUNCTION_VALUE_LIMIT as u32);
    JS_USER_FUNCTION_VALUE_BASE + offset as i32
}

pub(in crate::backend::direct_wasm) fn internal_function_name_hint(
    function_name: &str,
) -> Option<&str> {
    function_name
        .rsplit_once("__name_")
        .map(|(_, hinted_name)| hinted_name)
        .filter(|hinted_name| !hinted_name.is_empty())
}

pub(in crate::backend::direct_wasm) fn function_display_name(
    function: &FunctionDeclaration,
) -> Option<String> {
    function
        .self_binding
        .clone()
        .or_else(|| function.top_level_binding.clone())
        .or_else(|| internal_function_name_hint(&function.name).map(str::to_string))
        .or_else(|| (!function.name.starts_with("__ayy_")).then(|| function.name.clone()))
}

pub(in crate::backend::direct_wasm) fn builtin_function_display_name(name: &str) -> &str {
    match name {
        FUNCTION_CONSTRUCTOR_FAMILY_BUILTIN => "Function",
        _ => name.rsplit('.').next().unwrap_or(name),
    }
}

pub(in crate::backend::direct_wasm) fn builtin_function_length(name: &str) -> Option<u32> {
    match name {
        "Math.atan" | "Math.exp" => Some(1),
        "Math.max" | "Math.min" => Some(2),
        _ => None,
    }
}
