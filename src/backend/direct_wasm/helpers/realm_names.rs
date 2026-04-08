use super::*;

pub(in crate::backend::direct_wasm) fn test262_realm_identifier(id: u32) -> String {
    format!("{TEST262_REALM_IDENTIFIER_PREFIX}{id}")
}

pub(in crate::backend::direct_wasm) fn test262_realm_global_identifier(id: u32) -> String {
    format!("{TEST262_REALM_GLOBAL_IDENTIFIER_PREFIX}{id}")
}

pub(in crate::backend::direct_wasm) fn test262_realm_eval_builtin_name(id: u32) -> String {
    format!("{TEST262_REALM_EVAL_BUILTIN_PREFIX}{id}")
}

fn parse_prefixed_u32(name: &str, prefix: &str) -> Option<u32> {
    name.strip_prefix(prefix)?.parse::<u32>().ok()
}

pub(in crate::backend::direct_wasm) fn parse_test262_realm_identifier(name: &str) -> Option<u32> {
    parse_prefixed_u32(name, TEST262_REALM_IDENTIFIER_PREFIX)
}

pub(in crate::backend::direct_wasm) fn parse_test262_realm_global_identifier(
    name: &str,
) -> Option<u32> {
    parse_prefixed_u32(name, TEST262_REALM_GLOBAL_IDENTIFIER_PREFIX)
}

pub(in crate::backend::direct_wasm) fn parse_test262_realm_eval_builtin(name: &str) -> Option<u32> {
    parse_prefixed_u32(name, TEST262_REALM_EVAL_BUILTIN_PREFIX)
}
