use super::*;

const BOUND_FUNCTION_PROTOTYPE_CALL_PREFIX: &str = "__ayy_bound_call__";

pub(in crate::backend::direct_wasm) fn builtin_prototype_function_name(
    object_name: &str,
    property_name: &str,
) -> Option<&'static str> {
    match (object_name, property_name) {
        ("Function", "call") => Some("Function.prototype.call"),
        ("Function", "apply") => Some("Function.prototype.apply"),
        ("Function", "bind") => Some("Function.prototype.bind"),
        ("Array", "join") => Some("Array.prototype.join"),
        ("Array", "push") => Some("Array.prototype.push"),
        ("Object", "hasOwnProperty") => Some("Object.prototype.hasOwnProperty"),
        ("Object", "propertyIsEnumerable") => Some("Object.prototype.propertyIsEnumerable"),
        ("Object", "__lookupGetter__") => Some("Object.prototype.__lookupGetter__"),
        ("Object", "__lookupSetter__") => Some("Object.prototype.__lookupSetter__"),
        _ => None,
    }
}

pub(in crate::backend::direct_wasm) fn bound_function_prototype_call_builtin_name(
    target_name: &str,
) -> String {
    format!("{BOUND_FUNCTION_PROTOTYPE_CALL_PREFIX}{target_name}")
}

pub(in crate::backend::direct_wasm) fn parse_bound_function_prototype_call_builtin_name(
    name: &str,
) -> Option<&str> {
    name.strip_prefix(BOUND_FUNCTION_PROTOTYPE_CALL_PREFIX)
}

pub(in crate::backend::direct_wasm) fn builtin_member_function_name(
    object_name: &str,
    property_name: &str,
) -> Option<&'static str> {
    match (object_name, property_name) {
        ("Array", "isArray") => Some("Array.isArray"),
        ("JSON", "stringify") => Some("JSON.stringify"),
        ("Object", "create") => Some("Object.create"),
        ("Object", "getOwnPropertyDescriptor") => Some("Object.getOwnPropertyDescriptor"),
        ("Object", "getOwnPropertyNames") => Some("Object.getOwnPropertyNames"),
        ("Object", "getOwnPropertySymbols") => Some("Object.getOwnPropertySymbols"),
        ("Object", "getPrototypeOf") => Some("Object.getPrototypeOf"),
        ("Object", "is") => Some("Object.is"),
        ("Object", "isExtensible") => Some("Object.isExtensible"),
        ("Object", "keys") => Some("Object.keys"),
        ("Object", "setPrototypeOf") => Some("Object.setPrototypeOf"),
        ("Promise", "resolve") => Some("Promise.resolve"),
        ("Promise", "reject") => Some("Promise.reject"),
        ("Math", "atan") => Some("Math.atan"),
        ("Math", "exp") => Some("Math.exp"),
        ("Math", "max") => Some("Math.max"),
        ("Math", "min") => Some("Math.min"),
        _ => None,
    }
}

pub(in crate::backend::direct_wasm) fn builtin_member_number_value(
    object_name: &str,
    property_name: &str,
) -> Option<f64> {
    match (object_name, property_name) {
        ("Math", "E") => Some(std::f64::consts::E),
        ("Math", "LN2") => Some(std::f64::consts::LN_2),
        ("Math", "LN10") => Some(std::f64::consts::LN_10),
        ("Math", "LOG2E") => Some(std::f64::consts::LOG2_E),
        ("Math", "LOG10E") => Some(std::f64::consts::LOG10_E),
        ("Math", "PI") => Some(std::f64::consts::PI),
        ("Math", "SQRT1_2") => Some(std::f64::consts::FRAC_1_SQRT_2),
        ("Math", "SQRT2") => Some(std::f64::consts::SQRT_2),
        _ => None,
    }
}

pub(in crate::backend::direct_wasm) fn builtin_function_runtime_value(name: &str) -> Option<i32> {
    match name {
        "eval" => Some(JS_BUILTIN_EVAL_VALUE),
        TEST262_CREATE_REALM_BUILTIN => Some(JS_TYPEOF_FUNCTION_TAG),
        _ => None,
    }
    .or_else(|| parse_test262_realm_eval_builtin(name).map(|_| JS_TYPEOF_FUNCTION_TAG))
}

pub(in crate::backend::direct_wasm) fn is_non_definable_global_name(name: &str) -> bool {
    matches!(name, "NaN" | "Infinity" | "undefined")
}
