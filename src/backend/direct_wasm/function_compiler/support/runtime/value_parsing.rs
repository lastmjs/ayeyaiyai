use super::*;

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
