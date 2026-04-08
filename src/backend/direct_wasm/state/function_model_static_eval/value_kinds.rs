use super::super::super::*;

#[derive(Clone, Copy, PartialEq)]
pub(in crate::backend::direct_wasm) enum StaticValueKind {
    Unknown,
    Number,
    Bool,
    String,
    Object,
    BigInt,
    Null,
    Undefined,
    Function,
    Symbol,
}

impl StaticValueKind {
    pub(in crate::backend::direct_wasm) fn as_typeof_str(self) -> Option<&'static str> {
        match self {
            StaticValueKind::Number => Some("number"),
            StaticValueKind::Bool => Some("boolean"),
            StaticValueKind::String => Some("string"),
            StaticValueKind::Object => Some("object"),
            StaticValueKind::BigInt => Some("bigint"),
            StaticValueKind::Function => Some("function"),
            StaticValueKind::Symbol => Some("symbol"),
            StaticValueKind::Null => Some("object"),
            StaticValueKind::Undefined => Some("undefined"),
            StaticValueKind::Unknown => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn as_typeof_tag(self) -> Option<i32> {
        match self {
            StaticValueKind::Number => Some(JS_TYPEOF_NUMBER_TAG),
            StaticValueKind::Bool => Some(JS_TYPEOF_BOOLEAN_TAG),
            StaticValueKind::String => Some(JS_TYPEOF_STRING_TAG),
            StaticValueKind::Object => Some(JS_TYPEOF_OBJECT_TAG),
            StaticValueKind::BigInt => Some(JS_TYPEOF_BIGINT_TAG),
            StaticValueKind::Function => Some(JS_TYPEOF_FUNCTION_TAG),
            StaticValueKind::Symbol => Some(JS_TYPEOF_SYMBOL_TAG),
            StaticValueKind::Null => Some(JS_TYPEOF_OBJECT_TAG),
            StaticValueKind::Undefined => Some(JS_TYPEOF_UNDEFINED_TAG),
            StaticValueKind::Unknown => None,
        }
    }
}
