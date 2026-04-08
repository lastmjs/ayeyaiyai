use super::*;

pub(in crate::backend::direct_wasm) fn infer_call_result_kind(
    name: &str,
) -> Option<StaticValueKind> {
    match name {
        "Number" => Some(StaticValueKind::Number),
        "String" => Some(StaticValueKind::String),
        "Boolean" => Some(StaticValueKind::Bool),
        "isNaN" => Some(StaticValueKind::Bool),
        "Object" | "Array" | "ArrayBuffer" | "Date" | "RegExp" | "Map" | "Set" | "Error"
        | "EvalError" | "RangeError" | "ReferenceError" | "SyntaxError" | "TypeError"
        | "URIError" | "AggregateError" | "Promise" | "WeakRef" => Some(StaticValueKind::Object),
        "Uint8Array" | "Int8Array" | "Uint16Array" | "Int16Array" | "Uint32Array"
        | "Int32Array" | "Float32Array" | "Float64Array" | "Uint8ClampedArray" => {
            Some(StaticValueKind::Object)
        }
        "BigInt" => Some(StaticValueKind::BigInt),
        "Symbol" => Some(StaticValueKind::Symbol),
        "Function" | FUNCTION_CONSTRUCTOR_FAMILY_BUILTIN | "eval" => {
            Some(StaticValueKind::Function)
        }
        _ => None,
    }
}

pub(in crate::backend::direct_wasm) fn is_builtin_like_capture_identifier(name: &str) -> bool {
    name == "eval"
        || name == "console"
        || matches!(
            name,
            "__assert" | "__assertSameValue" | "__assertNotSameValue" | "__ayyAssertThrows"
        )
        || builtin_identifier_kind(name).is_some()
        || infer_call_result_kind(name).is_some()
}

pub(in crate::backend::direct_wasm) fn is_function_constructor_builtin(name: &str) -> bool {
    matches!(
        name,
        "Function"
            | "AsyncFunction"
            | "GeneratorFunction"
            | "AsyncGeneratorFunction"
            | FUNCTION_CONSTRUCTOR_FAMILY_BUILTIN
    )
}

pub(in crate::backend::direct_wasm) fn are_function_constructor_bindings(
    bindings: &[LocalFunctionBinding],
) -> bool {
    bindings.iter().all(|binding| {
        matches!(
            binding,
            LocalFunctionBinding::Builtin(name) if is_function_constructor_builtin(name)
        )
    })
}

pub(in crate::backend::direct_wasm) fn typed_array_builtin_bytes_per_element(
    name: &str,
) -> Option<u32> {
    match name {
        "Uint8Array" | "Int8Array" | "Uint8ClampedArray" => Some(1),
        "Uint16Array" | "Int16Array" => Some(2),
        "Uint32Array" | "Int32Array" | "Float32Array" => Some(4),
        "Float64Array" => Some(8),
        _ => None,
    }
}

pub(in crate::backend::direct_wasm) fn native_error_runtime_value(name: &str) -> Option<i32> {
    NATIVE_ERROR_NAMES
        .iter()
        .position(|candidate| *candidate == name)
        .map(|offset| JS_NATIVE_ERROR_VALUE_BASE + offset as i32)
}

pub(in crate::backend::direct_wasm) fn native_error_instanceof_values(
    name: &str,
) -> Option<Vec<i32>> {
    if name == "Error" {
        return Some(
            NATIVE_ERROR_NAMES
                .iter()
                .filter_map(|candidate| native_error_runtime_value(candidate))
                .collect(),
        );
    }

    native_error_runtime_value(name).map(|value| vec![value])
}
