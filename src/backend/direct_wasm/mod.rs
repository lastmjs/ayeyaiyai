use std::{
    borrow::Cow,
    cell::RefCell,
    collections::{BTreeSet, HashMap, HashSet},
};

use anyhow::Result;
use num_bigint::BigInt as StaticBigInt;

use crate::{
    frontend,
    ir::hir::{
        ArrayElement, BinaryOp, CallArgument, Expression, FunctionDeclaration, FunctionKind,
        ObjectEntry, Program, Statement, UnaryOp, UpdateOp,
    },
};

mod analysis;
mod encoding;
mod function_compiler;
mod program_compiler;

use self::{
    analysis::*,
    encoding::{
        encode_code_section, encode_data_section, encode_export_section, encode_function_section,
        encode_global_section, encode_import_section, encode_memory_section, encode_type_section,
        push_i32, push_section, push_u32,
    },
    function_compiler::*,
};

#[allow(dead_code)]
#[derive(Debug)]
struct Unsupported(&'static str);

type DirectResult<T> = std::result::Result<T, Unsupported>;

const WASM_MAGIC_AND_VERSION: &[u8] = b"\0asm\x01\0\0\0";
const I32_TYPE: u8 = 0x7f;
const EMPTY_BLOCK_TYPE: u8 = 0x40;
const JS_NULL_TAG: i32 = -1073741824;
const JS_UNDEFINED_TAG: i32 = -1073741823;
const JS_TYPEOF_NUMBER_TAG: i32 = -1073741822;
const JS_TYPEOF_STRING_TAG: i32 = -1073741821;
const JS_TYPEOF_BOOLEAN_TAG: i32 = -1073741820;
const JS_TYPEOF_OBJECT_TAG: i32 = -1073741819;
const JS_TYPEOF_UNDEFINED_TAG: i32 = -1073741818;
const JS_TYPEOF_FUNCTION_TAG: i32 = -1073741817;
const JS_TYPEOF_SYMBOL_TAG: i32 = -1073741816;
const JS_TYPEOF_BIGINT_TAG: i32 = -1073741815;
const JS_NAN_TAG: i32 = -1073741814;
const JS_BUILTIN_EVAL_VALUE: i32 = -1073539000;
const JS_NATIVE_ERROR_VALUE_BASE: i32 = -1073540000;
const JS_NATIVE_ERROR_VALUE_LIMIT: i32 = 8;
const JS_USER_FUNCTION_VALUE_BASE: i32 = -1073640000;
const JS_USER_FUNCTION_VALUE_LIMIT: i32 = 100000;
const FUNCTION_CONSTRUCTOR_FAMILY_BUILTIN: &str = "__ayyFunctionConstructorFamily";
const TEST262_CREATE_REALM_BUILTIN: &str = "__ayyTest262CreateRealm";
const TEST262_REALM_IDENTIFIER_PREFIX: &str = "__ayy_test262_realm_";
const TEST262_REALM_GLOBAL_IDENTIFIER_PREFIX: &str = "__ayy_test262_realm_global_";
const TEST262_REALM_EVAL_BUILTIN_PREFIX: &str = "__ayyTest262RealmEval";

const WASI_FD_WRITE_TYPE_INDEX: u32 = 0;
const WRITE_BYTES_TYPE_INDEX: u32 = 1;
const UNARY_VOID_TYPE_INDEX: u32 = 2;
const START_TYPE_INDEX: u32 = 3;
const USER_TYPE_BASE_INDEX: u32 = 4;

const FD_WRITE_FUNCTION_INDEX: u32 = 0;
const WRITE_BYTES_FUNCTION_INDEX: u32 = 1;
const WRITE_CHAR_FUNCTION_INDEX: u32 = 2;
const PRINT_U32_FUNCTION_INDEX: u32 = 3;
const PRINT_I32_FUNCTION_INDEX: u32 = 4;
const START_FUNCTION_INDEX: u32 = 5;
const USER_FUNCTION_BASE_INDEX: u32 = 6;

const THROW_TAG_GLOBAL_INDEX: u32 = 0;
const THROW_VALUE_GLOBAL_INDEX: u32 = 1;
const CURRENT_NEW_TARGET_GLOBAL_INDEX: u32 = 2;
const CURRENT_THIS_GLOBAL_INDEX: u32 = 3;
const TRACKED_ARGUMENT_SLOT_LIMIT: u32 = 64;
const TRACKED_ARRAY_SLOT_LIMIT: u32 = 32;

const IOVEC_OFFSET: u32 = 0;
const NWRITTEN_OFFSET: u32 = 8;
const CHAR_OFFSET: u32 = 16;
const DATA_START_OFFSET: u32 = 64;

const NATIVE_ERROR_NAMES: [&str; JS_NATIVE_ERROR_VALUE_LIMIT as usize] = [
    "Error",
    "EvalError",
    "RangeError",
    "ReferenceError",
    "SyntaxError",
    "TypeError",
    "URIError",
    "AggregateError",
];

fn empty_object_value_binding() -> ObjectValueBinding {
    ObjectValueBinding {
        string_properties: Vec::new(),
        symbol_properties: Vec::new(),
        non_enumerable_string_properties: Vec::new(),
    }
}

fn test262_realm_identifier(id: u32) -> String {
    format!("{TEST262_REALM_IDENTIFIER_PREFIX}{id}")
}

fn test262_realm_global_identifier(id: u32) -> String {
    format!("{TEST262_REALM_GLOBAL_IDENTIFIER_PREFIX}{id}")
}

fn test262_realm_eval_builtin_name(id: u32) -> String {
    format!("{TEST262_REALM_EVAL_BUILTIN_PREFIX}{id}")
}

fn parse_prefixed_u32(name: &str, prefix: &str) -> Option<u32> {
    name.strip_prefix(prefix)?.parse::<u32>().ok()
}

fn parse_test262_realm_identifier(name: &str) -> Option<u32> {
    parse_prefixed_u32(name, TEST262_REALM_IDENTIFIER_PREFIX)
}

fn parse_test262_realm_global_identifier(name: &str) -> Option<u32> {
    parse_prefixed_u32(name, TEST262_REALM_GLOBAL_IDENTIFIER_PREFIX)
}

fn parse_test262_realm_eval_builtin(name: &str) -> Option<u32> {
    parse_prefixed_u32(name, TEST262_REALM_EVAL_BUILTIN_PREFIX)
}

#[derive(Default)]
struct DirectWasmCompiler {
    string_data: Vec<(u32, Vec<u8>)>,
    interned_strings: HashMap<Vec<u8>, (u32, u32)>,
    next_data_offset: u32,
    next_user_type_index: u32,
    user_type_indices: HashMap<u32, u32>,
    user_type_arities: Vec<u32>,
    user_functions: Vec<UserFunction>,
    registered_function_declarations: Vec<FunctionDeclaration>,
    user_function_map: HashMap<String, UserFunction>,
    user_function_parameter_bindings:
        HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
    user_function_parameter_value_bindings: HashMap<String, HashMap<String, Option<Expression>>>,
    user_function_parameter_array_bindings:
        HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
    user_function_parameter_object_bindings:
        HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
    global_bindings: HashMap<String, u32>,
    global_lexical_bindings: HashSet<String>,
    global_kinds: HashMap<String, StaticValueKind>,
    global_value_bindings: HashMap<String, Expression>,
    global_array_bindings: HashMap<String, ArrayValueBinding>,
    global_object_bindings: HashMap<String, ObjectValueBinding>,
    global_property_descriptors: HashMap<String, GlobalPropertyDescriptorState>,
    global_object_prototype_bindings: HashMap<String, Expression>,
    global_runtime_prototype_bindings: HashMap<String, GlobalObjectRuntimePrototypeBinding>,
    global_prototype_object_bindings: HashMap<String, ObjectValueBinding>,
    global_arguments_bindings: HashMap<String, ArgumentsValueBinding>,
    global_function_bindings: HashMap<String, LocalFunctionBinding>,
    global_specialized_function_values: HashMap<String, SpecializedFunctionValue>,
    implicit_global_bindings: HashMap<String, ImplicitGlobalBinding>,
    global_proxy_bindings: HashMap<String, ProxyValueBinding>,
    global_member_function_bindings: HashMap<MemberFunctionBindingKey, LocalFunctionBinding>,
    global_member_getter_bindings: HashMap<MemberFunctionBindingKey, LocalFunctionBinding>,
    global_member_setter_bindings: HashMap<MemberFunctionBindingKey, LocalFunctionBinding>,
    eval_local_function_bindings: HashMap<String, HashMap<String, String>>,
    user_function_capture_bindings: HashMap<String, HashMap<String, String>>,
    next_test262_realm_id: u32,
    test262_realms: HashMap<u32, Test262Realm>,
}

#[derive(Clone)]
struct GlobalObjectRuntimePrototypeBinding {
    global_index: Option<u32>,
    variants: Vec<Option<Expression>>,
}

#[derive(Clone)]
struct Test262Realm {
    global_object_binding: ObjectValueBinding,
}

#[derive(Clone, Copy)]
struct ImplicitGlobalBinding {
    value_index: u32,
    present_index: u32,
}

#[derive(Clone, Copy)]
struct PreparedCaptureBinding {
    binding: ImplicitGlobalBinding,
    saved_value_local: u32,
    saved_present_local: u32,
}

#[derive(Clone, Copy, PartialEq)]
enum StaticValueKind {
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
    fn as_typeof_str(self) -> Option<&'static str> {
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

    fn as_typeof_tag(self) -> Option<i32> {
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

#[derive(Clone, Debug, PartialEq)]
enum LocalFunctionBinding {
    User(String),
    Builtin(String),
}

#[derive(Clone)]
struct ResolvedPropertyKey {
    key: Expression,
    coercion: Option<LocalFunctionBinding>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum ReturnedMemberFunctionBindingTarget {
    Value,
    Prototype,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct ReturnedMemberFunctionBindingKey {
    target: ReturnedMemberFunctionBindingTarget,
    property: String,
}

#[derive(Clone)]
struct ReturnedMemberFunctionBinding {
    target: ReturnedMemberFunctionBindingTarget,
    property: String,
    binding: LocalFunctionBinding,
}

#[derive(Clone)]
struct ReturnedMemberValueBinding {
    property: String,
    value: Expression,
}

#[derive(Clone)]
enum InlineFunctionEffect {
    Assign {
        name: String,
        value: Expression,
    },
    Update {
        name: String,
        op: UpdateOp,
        prefix: bool,
    },
    Expression(Expression),
}

#[derive(Clone, Default)]
struct InlineFunctionSummary {
    effects: Vec<InlineFunctionEffect>,
    return_value: Option<Expression>,
}

#[derive(Clone)]
struct SpecializedFunctionValue {
    binding: LocalFunctionBinding,
    summary: InlineFunctionSummary,
}

#[derive(Clone)]
enum StaticThrowValue {
    Value(Expression),
    NamedError(&'static str),
}

#[derive(Clone)]
enum StaticEvalOutcome {
    Value(Expression),
    Throw(StaticThrowValue),
}

#[derive(Clone, Copy)]
enum PrimitiveHint {
    Default,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SymbolToPrimitiveHandling {
    NotHandled,
    Handled,
    AlwaysThrows,
}

#[derive(Clone)]
struct OrdinaryToPrimitiveStep {
    binding: LocalFunctionBinding,
    outcome: StaticEvalOutcome,
}

#[derive(Clone)]
struct OrdinaryToPrimitivePlan {
    steps: Vec<OrdinaryToPrimitiveStep>,
}

#[derive(Clone, Copy)]
enum OrdinaryToPrimitiveAnalysis {
    Unknown,
    Primitive(StaticValueKind),
    Throw,
    TypeError,
}

#[derive(Clone)]
struct UserFunction {
    name: String,
    kind: FunctionKind,
    params: Vec<String>,
    scope_bindings: HashSet<String>,
    parameter_defaults: Vec<Option<Expression>>,
    body_declares_arguments_binding: bool,
    length: u32,
    extra_argument_indices: Vec<u32>,
    enumerated_keys_param_index: Option<usize>,
    returns_arguments_object: bool,
    returned_arguments_effects: ReturnedArgumentsEffects,
    returned_member_function_bindings: Vec<ReturnedMemberFunctionBinding>,
    returned_member_value_bindings: Vec<ReturnedMemberValueBinding>,
    inline_summary: Option<InlineFunctionSummary>,
    home_object_binding: Option<String>,
    strict: bool,
    lexical_this: bool,
    function_index: u32,
    type_index: u32,
}

impl UserFunction {
    fn is_async(&self) -> bool {
        matches!(self.kind, FunctionKind::Async)
    }

    fn is_arrow(&self) -> bool {
        self.lexical_this
    }

    fn is_generator(&self) -> bool {
        matches!(self.kind, FunctionKind::Generator)
    }

    fn is_constructible(&self) -> bool {
        matches!(self.kind, FunctionKind::Ordinary) && !self.lexical_this
    }

    fn visible_param_count(&self) -> u32 {
        self.params.len() as u32
    }

    fn has_parameter_defaults(&self) -> bool {
        self.parameter_defaults.iter().any(Option::is_some)
    }

    fn wasm_param_count(&self) -> u32 {
        self.visible_param_count() + 1 + self.extra_argument_indices.len() as u32
    }

    fn actual_argument_count_param(&self) -> u32 {
        self.visible_param_count()
    }

    fn extra_argument_param(&self, index: u32) -> Option<u32> {
        let position = self
            .extra_argument_indices
            .iter()
            .position(|candidate| *candidate == index)? as u32;
        Some(self.visible_param_count() + 1 + position)
    }
}

struct CompiledFunction {
    local_count: u32,
    instructions: Vec<u8>,
}

#[derive(Default)]
struct ArgumentsUsage {
    indexed_slots: Vec<u32>,
}

#[derive(Clone)]
struct ArgumentsIndexedPropertyState {
    present: bool,
    mapped: bool,
    writable: bool,
    enumerable: bool,
    configurable: bool,
    getter: Option<Expression>,
    setter: Option<Expression>,
}

impl ArgumentsIndexedPropertyState {
    fn data(present: bool, mapped: bool) -> Self {
        Self {
            present,
            mapped,
            writable: true,
            enumerable: true,
            configurable: true,
            getter: None,
            setter: None,
        }
    }

    fn is_accessor(&self) -> bool {
        self.getter.is_some() || self.setter.is_some()
    }
}

#[derive(Clone)]
struct ArgumentsSlot {
    value_local: u32,
    present_local: u32,
    mapped_local: Option<u32>,
    source_param_local: Option<u32>,
    state: ArgumentsIndexedPropertyState,
}

#[derive(Clone)]
struct ArgumentsValueBinding {
    values: Vec<Expression>,
    strict: bool,
    callee_present: bool,
    callee_value: Option<Expression>,
    length_present: bool,
    length_value: Expression,
}

#[derive(Clone, PartialEq)]
struct ArrayValueBinding {
    values: Vec<Option<Expression>>,
}

#[derive(Clone)]
struct RuntimeArraySlot {
    value_local: u32,
    present_local: u32,
}

#[derive(Clone)]
struct ResizableArrayBufferBinding {
    values: Vec<Option<Expression>>,
    max_length: usize,
}

#[derive(Clone)]
struct TypedArrayViewBinding {
    buffer_name: String,
    offset: usize,
    fixed_length: Option<usize>,
}

#[derive(Clone, PartialEq)]
struct ObjectValueBinding {
    string_properties: Vec<(String, Expression)>,
    symbol_properties: Vec<(Expression, Expression)>,
    non_enumerable_string_properties: Vec<String>,
}

#[derive(Clone)]
enum IteratorSourceKind {
    StaticArray {
        values: Vec<Option<Expression>>,
        keys_only: bool,
        length_local: Option<u32>,
        runtime_name: Option<String>,
    },
    SimpleGenerator {
        steps: Vec<SimpleGeneratorStep>,
        completion_effects: Vec<Statement>,
    },
    TypedArrayView {
        name: String,
    },
    DirectArguments {
        tracked_prefix_len: u32,
    },
}

#[derive(Clone)]
struct ArrayIteratorBinding {
    source: IteratorSourceKind,
    index_local: u32,
    static_index: Option<usize>,
}

#[derive(Clone)]
struct SimpleGeneratorStep {
    effects: Vec<Statement>,
    outcome: SimpleGeneratorStepOutcome,
}

#[derive(Clone)]
enum SimpleGeneratorStepOutcome {
    Yield(Expression),
    Throw(Expression),
}

#[derive(Clone)]
enum IteratorStepBinding {
    Runtime {
        done_local: u32,
        value_local: u32,
        function_binding: Option<LocalFunctionBinding>,
        static_done: Option<bool>,
        static_value: Option<Expression>,
    },
}

#[derive(Clone)]
struct ProxyValueBinding {
    target: Expression,
    has_binding: Option<LocalFunctionBinding>,
}

impl ArgumentsValueBinding {
    fn for_user_function(user_function: &UserFunction, values: Vec<Expression>) -> Self {
        let mut binding = Self {
            length_value: Expression::Number(values.len() as f64),
            values,
            strict: user_function.strict,
            callee_present: true,
            callee_value: if user_function.strict {
                None
            } else {
                Some(Expression::Identifier(user_function.name.clone()))
            },
            length_present: true,
        };
        binding.apply_effects(&user_function.returned_arguments_effects);
        binding
    }

    fn apply_effects(&mut self, effects: &ReturnedArgumentsEffects) {
        if let Some(effect) = &effects.callee {
            self.apply_named_effect("callee", effect.clone());
        }
        if let Some(effect) = &effects.length {
            self.apply_named_effect("length", effect.clone());
        }
    }

    fn apply_named_effect(&mut self, property_name: &str, effect: ArgumentsPropertyEffect) {
        match property_name {
            "callee" => {
                if self.strict {
                    return;
                }
                match effect {
                    ArgumentsPropertyEffect::Assign(value) => {
                        self.callee_present = true;
                        self.callee_value = Some(value);
                    }
                    ArgumentsPropertyEffect::Delete => {
                        self.callee_present = false;
                        self.callee_value = None;
                    }
                }
            }
            "length" => match effect {
                ArgumentsPropertyEffect::Assign(value) => {
                    self.length_present = true;
                    self.length_value = value;
                }
                ArgumentsPropertyEffect::Delete => {
                    self.length_present = false;
                    self.length_value = Expression::Undefined;
                }
            },
            _ => {}
        }
    }
}

#[derive(Clone, Default)]
struct ReturnedArgumentsEffects {
    callee: Option<ArgumentsPropertyEffect>,
    length: Option<ArgumentsPropertyEffect>,
}

#[derive(Clone)]
enum ArgumentsPropertyEffect {
    Assign(Expression),
    Delete,
}

#[derive(Clone)]
struct PropertyDescriptorBinding {
    value: Option<Expression>,
    configurable: bool,
    enumerable: bool,
    writable: Option<bool>,
    has_get: bool,
    has_set: bool,
}

#[derive(Clone)]
struct GlobalPropertyDescriptorState {
    value: Expression,
    writable: Option<bool>,
    enumerable: bool,
    configurable: bool,
}

#[derive(Clone)]
enum StringConcatFragment {
    Static(String),
    Dynamic(Expression),
}

#[derive(Clone, Default)]
struct PropertyDescriptorDefinition {
    value: Option<Expression>,
    writable: Option<bool>,
    enumerable: Option<bool>,
    configurable: Option<bool>,
    getter: Option<Expression>,
    setter: Option<Expression>,
}

impl PropertyDescriptorDefinition {
    fn is_accessor(&self) -> bool {
        self.getter.is_some() || self.setter.is_some()
    }
}

#[derive(Clone)]
struct LoopContext {
    break_target: usize,
    continue_target: usize,
    labels: Vec<String>,
}

#[derive(Clone)]
struct BreakContext {
    break_target: usize,
    labels: Vec<String>,
    break_hook: Option<Expression>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum MemberFunctionBindingTarget {
    Identifier(String),
    Prototype(String),
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum MemberFunctionBindingProperty {
    String(String),
    Symbol(String),
    SymbolExpression(String),
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct MemberFunctionBindingKey {
    target: MemberFunctionBindingTarget,
    property: MemberFunctionBindingProperty,
}

struct MaterializationGuard<'a> {
    active: &'a RefCell<HashSet<usize>>,
    key: usize,
}

impl Drop for MaterializationGuard<'_> {
    fn drop(&mut self) {
        self.active.borrow_mut().remove(&self.key);
    }
}

struct FunctionCompiler<'a> {
    module: &'a mut DirectWasmCompiler,
    parameter_names: Vec<String>,
    parameter_defaults: Vec<Option<Expression>>,
    parameter_initialized_locals: HashMap<String, u32>,
    parameter_scope_arguments_local: Option<u32>,
    in_parameter_default_initialization: bool,
    locals: HashMap<String, u32>,
    local_kinds: HashMap<String, StaticValueKind>,
    local_value_bindings: HashMap<String, Expression>,
    local_function_bindings: HashMap<String, LocalFunctionBinding>,
    local_specialized_function_values: HashMap<String, SpecializedFunctionValue>,
    local_proxy_bindings: HashMap<String, ProxyValueBinding>,
    member_function_bindings: HashMap<MemberFunctionBindingKey, LocalFunctionBinding>,
    member_getter_bindings: HashMap<MemberFunctionBindingKey, LocalFunctionBinding>,
    member_setter_bindings: HashMap<MemberFunctionBindingKey, LocalFunctionBinding>,
    local_array_bindings: HashMap<String, ArrayValueBinding>,
    local_resizable_array_buffer_bindings: HashMap<String, ResizableArrayBufferBinding>,
    local_typed_array_view_bindings: HashMap<String, TypedArrayViewBinding>,
    runtime_typed_array_oob_locals: HashMap<String, u32>,
    tracked_array_function_values: HashMap<String, HashMap<u32, SpecializedFunctionValue>>,
    runtime_array_slots: HashMap<String, HashMap<u32, RuntimeArraySlot>>,
    local_array_iterator_bindings: HashMap<String, ArrayIteratorBinding>,
    local_iterator_step_bindings: HashMap<String, IteratorStepBinding>,
    runtime_array_length_locals: HashMap<String, u32>,
    materializing_expression_keys: RefCell<HashSet<usize>>,
    local_object_bindings: HashMap<String, ObjectValueBinding>,
    local_prototype_object_bindings: HashMap<String, ObjectValueBinding>,
    local_arguments_bindings: HashMap<String, ArgumentsValueBinding>,
    direct_arguments_aliases: HashSet<String>,
    local_descriptor_bindings: HashMap<String, PropertyDescriptorBinding>,
    eval_lexical_initialized_locals: HashMap<String, u32>,
    throw_tag_local: u32,
    throw_value_local: u32,
    strict_mode: bool,
    next_local_index: u32,
    param_count: u32,
    visible_param_count: u32,
    actual_argument_count_local: Option<u32>,
    extra_argument_param_locals: HashMap<u32, u32>,
    arguments_slots: HashMap<u32, ArgumentsSlot>,
    mapped_arguments: bool,
    current_user_function_name: Option<String>,
    current_arguments_callee_present: bool,
    current_arguments_callee_override: Option<Expression>,
    current_arguments_length_present: bool,
    current_arguments_length_override: Option<Expression>,
    instructions: Vec<u8>,
    control_stack: Vec<()>,
    loop_stack: Vec<LoopContext>,
    break_stack: Vec<BreakContext>,
    active_eval_lexical_scopes: Vec<Vec<(String, Option<String>)>>,
    active_eval_lexical_binding_counts: HashMap<String, u32>,
    active_scoped_lexical_bindings: HashMap<String, Vec<String>>,
    with_scopes: Vec<Expression>,
    try_stack: Vec<TryContext>,
    allow_return: bool,
    top_level_function: bool,
    isolated_indirect_eval: bool,
}

#[derive(Clone)]
struct TryContext {
    catch_target: usize,
}

pub(in crate::backend) fn try_emit_wasm(program: &Program) -> Result<Option<Vec<u8>>> {
    let mut compiler = DirectWasmCompiler::default();
    match compiler.compile(program) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(Unsupported(_)) => Ok(None),
    }
}

pub(in crate::backend) fn emit_wasm_with_reason(
    program: &Program,
) -> std::result::Result<Vec<u8>, &'static str> {
    let mut compiler = DirectWasmCompiler::default();
    compiler
        .compile(program)
        .map_err(|Unsupported(message)| message)
}
