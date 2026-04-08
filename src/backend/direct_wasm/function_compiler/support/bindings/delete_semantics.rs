use super::*;

pub(in crate::backend::direct_wasm) fn builtin_identifier_delete_returns_true(name: &str) -> bool {
    builtin_identifier_kind(name).is_some() && !matches!(name, "Infinity" | "NaN" | "undefined")
}

pub(in crate::backend::direct_wasm) fn builtin_member_delete_returns_false(
    object_name: &str,
    property_name: &str,
) -> bool {
    object_name == "Math" && builtin_member_number_value(object_name, property_name).is_some()
}
