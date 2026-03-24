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

pub(in crate::backend::direct_wasm) fn builtin_member_function_name(
    object_name: &str,
    property_name: &str,
) -> Option<&'static str> {
    match (object_name, property_name) {
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

pub(in crate::backend::direct_wasm) fn is_reserved_js_runtime_value(integer: i64) -> bool {
    integer == JS_NULL_TAG as i64
        || integer == JS_UNDEFINED_TAG as i64
        || integer == JS_TYPEOF_NUMBER_TAG as i64
        || integer == JS_TYPEOF_STRING_TAG as i64
        || integer == JS_TYPEOF_BOOLEAN_TAG as i64
        || integer == JS_TYPEOF_OBJECT_TAG as i64
        || integer == JS_TYPEOF_UNDEFINED_TAG as i64
        || integer == JS_TYPEOF_FUNCTION_TAG as i64
        || integer == JS_TYPEOF_SYMBOL_TAG as i64
        || integer == JS_TYPEOF_BIGINT_TAG as i64
        || integer == JS_BUILTIN_EVAL_VALUE as i64
        || (integer >= JS_NATIVE_ERROR_VALUE_BASE as i64
            && integer < (JS_NATIVE_ERROR_VALUE_BASE + JS_NATIVE_ERROR_VALUE_LIMIT) as i64)
        || (integer >= JS_USER_FUNCTION_VALUE_BASE as i64
            && integer < (JS_USER_FUNCTION_VALUE_BASE + JS_USER_FUNCTION_VALUE_LIMIT) as i64)
}

pub(in crate::backend::direct_wasm) fn f64_to_i32(value: f64) -> DirectResult<i32> {
    if !value.is_finite() || value.fract() != 0.0 {
        return Ok(0);
    }

    let integer = value as i64;
    if integer < i32::MIN as i64 || integer > i32::MAX as i64 {
        return Ok(0);
    }
    if is_reserved_js_runtime_value(integer) {
        return Err(Unsupported("number literal collides with reserved JS tag"));
    }

    Ok(integer as i32)
}

pub(in crate::backend::direct_wasm) fn parse_bigint_to_i32(value: &str) -> DirectResult<i32> {
    let literal = value.strip_suffix('n').unwrap_or(value);
    let integer = literal.parse::<i64>().unwrap_or(0);

    if integer < i32::MIN as i64 || integer > i32::MAX as i64 {
        return Ok(0);
    }
    if is_reserved_js_runtime_value(integer) {
        return Err(Unsupported("bigint literal collides with reserved JS tag"));
    }

    Ok(integer as i32)
}

pub(in crate::backend::direct_wasm) fn parse_static_bigint_literal(
    value: &str,
) -> Option<StaticBigInt> {
    let literal = value.strip_suffix('n').unwrap_or(value);
    let (negative, magnitude) = if let Some(rest) = literal.strip_prefix('-') {
        (true, rest)
    } else if let Some(rest) = literal.strip_prefix('+') {
        (false, rest)
    } else {
        (false, literal)
    };
    let (radix, digits) = if let Some(rest) = magnitude
        .strip_prefix("0x")
        .or_else(|| magnitude.strip_prefix("0X"))
    {
        (16, rest)
    } else if let Some(rest) = magnitude
        .strip_prefix("0o")
        .or_else(|| magnitude.strip_prefix("0O"))
    {
        (8, rest)
    } else if let Some(rest) = magnitude
        .strip_prefix("0b")
        .or_else(|| magnitude.strip_prefix("0B"))
    {
        (2, rest)
    } else {
        (10, magnitude)
    };
    let parsed = StaticBigInt::parse_bytes(digits.as_bytes(), radix)?;
    Some(if negative { -parsed } else { parsed })
}

pub(in crate::backend::direct_wasm) fn parse_string_to_loose_i32(value: &str) -> DirectResult<i32> {
    if let Some(type_tag) = parse_typeof_tag_optional(value) {
        return Ok(type_tag);
    }

    parse_string_to_i32(value)
}

pub(in crate::backend::direct_wasm) fn parse_typeof_tag_optional(value: &str) -> Option<i32> {
    match parse_typeof_tag(value) {
        Ok(tag) => Some(tag),
        Err(_) => None,
    }
}

pub(in crate::backend::direct_wasm) fn parse_typeof_tag(value: &str) -> DirectResult<i32> {
    match value.trim() {
        "number" => Ok(JS_TYPEOF_NUMBER_TAG),
        "string" => Ok(JS_TYPEOF_STRING_TAG),
        "boolean" => Ok(JS_TYPEOF_BOOLEAN_TAG),
        "object" => Ok(JS_TYPEOF_OBJECT_TAG),
        "undefined" => Ok(JS_TYPEOF_UNDEFINED_TAG),
        "function" => Ok(JS_TYPEOF_FUNCTION_TAG),
        "symbol" => Ok(JS_TYPEOF_SYMBOL_TAG),
        "bigint" => Ok(JS_TYPEOF_BIGINT_TAG),
        _ => Err(Unsupported("unknown typeof tag")),
    }
}

pub(in crate::backend::direct_wasm) fn parse_string_to_i32(value: &str) -> DirectResult<i32> {
    let trimmed = value.trim();

    let parsed = trimmed
        .parse::<i64>()
        .map_err(|_| Unsupported("non-numeric string literal"))?;

    if parsed < i32::MIN as i64 || parsed > i32::MAX as i64 {
        return Err(Unsupported("string literal integer is out of i32 range"));
    }
    if is_reserved_js_runtime_value(parsed) {
        return Err(Unsupported("string literal collides with reserved JS tag"));
    }

    Ok(parsed as i32)
}
