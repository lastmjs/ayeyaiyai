#[derive(Debug)]
pub(in crate::backend::direct_wasm) struct Unsupported(
    pub(in crate::backend::direct_wasm) &'static str,
);

pub(in crate::backend::direct_wasm) type DirectResult<T> = std::result::Result<T, Unsupported>;

pub(in crate::backend::direct_wasm) const WASM_MAGIC_AND_VERSION: &[u8] = b"\0asm\x01\0\0\0";
pub(in crate::backend::direct_wasm) const I32_TYPE: u8 = 0x7f;
pub(in crate::backend::direct_wasm) const EMPTY_BLOCK_TYPE: u8 = 0x40;
pub(in crate::backend::direct_wasm) const JS_NULL_TAG: i32 = -1073741824;
pub(in crate::backend::direct_wasm) const JS_UNDEFINED_TAG: i32 = -1073741823;
pub(in crate::backend::direct_wasm) const JS_TYPEOF_NUMBER_TAG: i32 = -1073741822;
pub(in crate::backend::direct_wasm) const JS_TYPEOF_STRING_TAG: i32 = -1073741821;
pub(in crate::backend::direct_wasm) const JS_TYPEOF_BOOLEAN_TAG: i32 = -1073741820;
pub(in crate::backend::direct_wasm) const JS_TYPEOF_OBJECT_TAG: i32 = -1073741819;
pub(in crate::backend::direct_wasm) const JS_TYPEOF_UNDEFINED_TAG: i32 = -1073741818;
pub(in crate::backend::direct_wasm) const JS_TYPEOF_FUNCTION_TAG: i32 = -1073741817;
pub(in crate::backend::direct_wasm) const JS_TYPEOF_SYMBOL_TAG: i32 = -1073741816;
pub(in crate::backend::direct_wasm) const JS_TYPEOF_BIGINT_TAG: i32 = -1073741815;
pub(in crate::backend::direct_wasm) const JS_NAN_TAG: i32 = -1073741814;
pub(in crate::backend::direct_wasm) const JS_BUILTIN_EVAL_VALUE: i32 = -1073539000;
pub(in crate::backend::direct_wasm) const JS_NATIVE_ERROR_VALUE_BASE: i32 = -1073540000;
pub(in crate::backend::direct_wasm) const JS_NATIVE_ERROR_VALUE_LIMIT: i32 = 8;
pub(in crate::backend::direct_wasm) const JS_USER_FUNCTION_VALUE_BASE: i32 = -1073640000;
pub(in crate::backend::direct_wasm) const JS_USER_FUNCTION_VALUE_LIMIT: i32 = 100000;
pub(in crate::backend::direct_wasm) const FUNCTION_CONSTRUCTOR_FAMILY_BUILTIN: &str =
    "__ayyFunctionConstructorFamily";
pub(in crate::backend::direct_wasm) const TEST262_CREATE_REALM_BUILTIN: &str =
    "__ayyTest262CreateRealm";
pub(in crate::backend::direct_wasm) const TEST262_REALM_IDENTIFIER_PREFIX: &str =
    "__ayy_test262_realm_";
pub(in crate::backend::direct_wasm) const TEST262_REALM_GLOBAL_IDENTIFIER_PREFIX: &str =
    "__ayy_test262_realm_global_";
pub(in crate::backend::direct_wasm) const TEST262_REALM_EVAL_BUILTIN_PREFIX: &str =
    "__ayyTest262RealmEval";

pub(in crate::backend::direct_wasm) const WASI_FD_WRITE_TYPE_INDEX: u32 = 0;
pub(in crate::backend::direct_wasm) const WRITE_BYTES_TYPE_INDEX: u32 = 1;
pub(in crate::backend::direct_wasm) const UNARY_VOID_TYPE_INDEX: u32 = 2;
pub(in crate::backend::direct_wasm) const START_TYPE_INDEX: u32 = 3;
pub(in crate::backend::direct_wasm) const USER_TYPE_BASE_INDEX: u32 = 4;

pub(in crate::backend::direct_wasm) const FD_WRITE_FUNCTION_INDEX: u32 = 0;
pub(in crate::backend::direct_wasm) const WRITE_BYTES_FUNCTION_INDEX: u32 = 1;
pub(in crate::backend::direct_wasm) const WRITE_CHAR_FUNCTION_INDEX: u32 = 2;
pub(in crate::backend::direct_wasm) const PRINT_U32_FUNCTION_INDEX: u32 = 3;
pub(in crate::backend::direct_wasm) const PRINT_I32_FUNCTION_INDEX: u32 = 4;
pub(in crate::backend::direct_wasm) const START_FUNCTION_INDEX: u32 = 5;
pub(in crate::backend::direct_wasm) const USER_FUNCTION_BASE_INDEX: u32 = 6;

pub(in crate::backend::direct_wasm) const THROW_TAG_GLOBAL_INDEX: u32 = 0;
pub(in crate::backend::direct_wasm) const THROW_VALUE_GLOBAL_INDEX: u32 = 1;
pub(in crate::backend::direct_wasm) const CURRENT_NEW_TARGET_GLOBAL_INDEX: u32 = 2;
pub(in crate::backend::direct_wasm) const CURRENT_THIS_GLOBAL_INDEX: u32 = 3;
pub(in crate::backend::direct_wasm) const TRACKED_ARGUMENT_SLOT_LIMIT: u32 = 64;
pub(in crate::backend::direct_wasm) const TRACKED_ARRAY_SLOT_LIMIT: u32 = 32;

pub(in crate::backend::direct_wasm) const IOVEC_OFFSET: u32 = 0;
pub(in crate::backend::direct_wasm) const NWRITTEN_OFFSET: u32 = 8;
pub(in crate::backend::direct_wasm) const CHAR_OFFSET: u32 = 16;
pub(in crate::backend::direct_wasm) const DATA_START_OFFSET: u32 = 64;
pub(in crate::backend::direct_wasm) const WASM_MEMORY_PAGE_SIZE: u32 = 65_536;

pub(in crate::backend::direct_wasm) const NATIVE_ERROR_NAMES: [&str;
    JS_NATIVE_ERROR_VALUE_LIMIT as usize] = [
    "Error",
    "EvalError",
    "RangeError",
    "ReferenceError",
    "SyntaxError",
    "TypeError",
    "URIError",
    "AggregateError",
];
