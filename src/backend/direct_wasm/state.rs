use std::ops::{Deref, DerefMut};

use super::*;

#[derive(Default)]
pub(in crate::backend::direct_wasm) struct DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) state: CompilerState,
}

impl Deref for DirectWasmCompiler {
    type Target = CompilerState;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl DerefMut for DirectWasmCompiler {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.state
    }
}

#[derive(Default)]
pub(in crate::backend::direct_wasm) struct CompilerState {
    pub(in crate::backend::direct_wasm) string_data: Vec<(u32, Vec<u8>)>,
    pub(in crate::backend::direct_wasm) interned_strings: HashMap<Vec<u8>, (u32, u32)>,
    pub(in crate::backend::direct_wasm) next_data_offset: u32,
    pub(in crate::backend::direct_wasm) next_user_type_index: u32,
    pub(in crate::backend::direct_wasm) user_type_indices: HashMap<u32, u32>,
    pub(in crate::backend::direct_wasm) user_type_arities: Vec<u32>,
    pub(in crate::backend::direct_wasm) user_functions: Vec<UserFunction>,
    pub(in crate::backend::direct_wasm) registered_function_declarations: Vec<FunctionDeclaration>,
    pub(in crate::backend::direct_wasm) user_function_map: HashMap<String, UserFunction>,
    pub(in crate::backend::direct_wasm) user_function_parameter_bindings:
        HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
    pub(in crate::backend::direct_wasm) user_function_parameter_value_bindings:
        HashMap<String, HashMap<String, Option<Expression>>>,
    pub(in crate::backend::direct_wasm) user_function_parameter_array_bindings:
        HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
    pub(in crate::backend::direct_wasm) user_function_parameter_object_bindings:
        HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
    pub(in crate::backend::direct_wasm) global_bindings: HashMap<String, u32>,
    pub(in crate::backend::direct_wasm) global_lexical_bindings: HashSet<String>,
    pub(in crate::backend::direct_wasm) global_kinds: HashMap<String, StaticValueKind>,
    pub(in crate::backend::direct_wasm) global_value_bindings: HashMap<String, Expression>,
    pub(in crate::backend::direct_wasm) global_array_bindings: HashMap<String, ArrayValueBinding>,
    pub(in crate::backend::direct_wasm) global_arrays_with_runtime_state: HashSet<String>,
    pub(in crate::backend::direct_wasm) global_object_bindings: HashMap<String, ObjectValueBinding>,
    pub(in crate::backend::direct_wasm) global_property_descriptors:
        HashMap<String, GlobalPropertyDescriptorState>,
    pub(in crate::backend::direct_wasm) global_object_prototype_bindings:
        HashMap<String, Expression>,
    pub(in crate::backend::direct_wasm) global_runtime_prototype_bindings:
        HashMap<String, GlobalObjectRuntimePrototypeBinding>,
    pub(in crate::backend::direct_wasm) global_prototype_object_bindings:
        HashMap<String, ObjectValueBinding>,
    pub(in crate::backend::direct_wasm) global_arguments_bindings:
        HashMap<String, ArgumentsValueBinding>,
    pub(in crate::backend::direct_wasm) global_function_bindings:
        HashMap<String, LocalFunctionBinding>,
    pub(in crate::backend::direct_wasm) global_specialized_function_values:
        HashMap<String, SpecializedFunctionValue>,
    pub(in crate::backend::direct_wasm) implicit_global_bindings:
        HashMap<String, ImplicitGlobalBinding>,
    pub(in crate::backend::direct_wasm) global_proxy_bindings: HashMap<String, ProxyValueBinding>,
    pub(in crate::backend::direct_wasm) global_member_function_bindings:
        HashMap<MemberFunctionBindingKey, LocalFunctionBinding>,
    pub(in crate::backend::direct_wasm) global_member_function_capture_slots:
        HashMap<MemberFunctionBindingKey, BTreeMap<String, String>>,
    pub(in crate::backend::direct_wasm) global_member_getter_bindings:
        HashMap<MemberFunctionBindingKey, LocalFunctionBinding>,
    pub(in crate::backend::direct_wasm) global_member_setter_bindings:
        HashMap<MemberFunctionBindingKey, LocalFunctionBinding>,
    pub(in crate::backend::direct_wasm) eval_local_function_bindings:
        HashMap<String, HashMap<String, String>>,
    pub(in crate::backend::direct_wasm) user_function_capture_bindings:
        HashMap<String, HashMap<String, String>>,
    pub(in crate::backend::direct_wasm) user_function_assigned_nonlocal_binding_results:
        HashMap<String, HashMap<String, Expression>>,
    pub(in crate::backend::direct_wasm) next_test262_realm_id: u32,
    pub(in crate::backend::direct_wasm) test262_realms: HashMap<u32, Test262Realm>,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct GlobalObjectRuntimePrototypeBinding {
    pub(in crate::backend::direct_wasm) global_index: Option<u32>,
    pub(in crate::backend::direct_wasm) variants: Vec<Option<Expression>>,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct Test262Realm {
    pub(in crate::backend::direct_wasm) global_object_binding: ObjectValueBinding,
}

#[derive(Clone, Copy)]
pub(in crate::backend::direct_wasm) struct ImplicitGlobalBinding {
    pub(in crate::backend::direct_wasm) value_index: u32,
    pub(in crate::backend::direct_wasm) present_index: u32,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct PreparedCaptureBinding {
    pub(in crate::backend::direct_wasm) binding: ImplicitGlobalBinding,
    pub(in crate::backend::direct_wasm) source_name: String,
    pub(in crate::backend::direct_wasm) hidden_name: String,
    pub(in crate::backend::direct_wasm) saved_value_local: u32,
    pub(in crate::backend::direct_wasm) saved_present_local: u32,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct PreparedBoundCaptureBinding {
    pub(in crate::backend::direct_wasm) binding: ImplicitGlobalBinding,
    pub(in crate::backend::direct_wasm) capture_name: String,
    pub(in crate::backend::direct_wasm) capture_hidden_name: String,
    pub(in crate::backend::direct_wasm) slot_name: String,
    pub(in crate::backend::direct_wasm) source_binding_name: Option<String>,
    pub(in crate::backend::direct_wasm) slot_local: u32,
    pub(in crate::backend::direct_wasm) saved_value_local: u32,
    pub(in crate::backend::direct_wasm) saved_present_local: u32,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct BoundUserFunctionCallSnapshot {
    pub(in crate::backend::direct_wasm) function_name: String,
    pub(in crate::backend::direct_wasm) source_expression: Option<Expression>,
    pub(in crate::backend::direct_wasm) result_expression: Option<Expression>,
    pub(in crate::backend::direct_wasm) updated_bindings: HashMap<String, Expression>,
}

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

#[derive(Clone, Debug, PartialEq)]
pub(in crate::backend::direct_wasm) enum LocalFunctionBinding {
    User(String),
    Builtin(String),
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct ResolvedPropertyKey {
    pub(in crate::backend::direct_wasm) key: Expression,
    pub(in crate::backend::direct_wasm) coercion: Option<LocalFunctionBinding>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::backend::direct_wasm) enum ReturnedMemberFunctionBindingTarget {
    Value,
    Prototype,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub(in crate::backend::direct_wasm) struct ReturnedMemberFunctionBindingKey {
    pub(in crate::backend::direct_wasm) target: ReturnedMemberFunctionBindingTarget,
    pub(in crate::backend::direct_wasm) property: String,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct ReturnedMemberFunctionBinding {
    pub(in crate::backend::direct_wasm) target: ReturnedMemberFunctionBindingTarget,
    pub(in crate::backend::direct_wasm) property: String,
    pub(in crate::backend::direct_wasm) binding: LocalFunctionBinding,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct ReturnedMemberValueBinding {
    pub(in crate::backend::direct_wasm) property: String,
    pub(in crate::backend::direct_wasm) value: Expression,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) enum InlineFunctionEffect {
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
pub(in crate::backend::direct_wasm) struct InlineFunctionSummary {
    pub(in crate::backend::direct_wasm) effects: Vec<InlineFunctionEffect>,
    pub(in crate::backend::direct_wasm) return_value: Option<Expression>,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct SpecializedFunctionValue {
    pub(in crate::backend::direct_wasm) binding: LocalFunctionBinding,
    pub(in crate::backend::direct_wasm) summary: InlineFunctionSummary,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) enum StaticThrowValue {
    Value(Expression),
    NamedError(&'static str),
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) enum StaticEvalOutcome {
    Value(Expression),
    Throw(StaticThrowValue),
}

#[derive(Clone, Copy)]
pub(in crate::backend::direct_wasm) enum PrimitiveHint {
    Default,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::direct_wasm) enum SymbolToPrimitiveHandling {
    NotHandled,
    Handled,
    AlwaysThrows,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct OrdinaryToPrimitiveStep {
    pub(in crate::backend::direct_wasm) binding: LocalFunctionBinding,
    pub(in crate::backend::direct_wasm) outcome: StaticEvalOutcome,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct OrdinaryToPrimitivePlan {
    pub(in crate::backend::direct_wasm) steps: Vec<OrdinaryToPrimitiveStep>,
}

#[derive(Clone, Copy)]
pub(in crate::backend::direct_wasm) enum OrdinaryToPrimitiveAnalysis {
    Unknown,
    Primitive(StaticValueKind),
    Throw,
    TypeError,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct UserFunction {
    pub(in crate::backend::direct_wasm) name: String,
    pub(in crate::backend::direct_wasm) kind: FunctionKind,
    pub(in crate::backend::direct_wasm) params: Vec<String>,
    pub(in crate::backend::direct_wasm) scope_bindings: HashSet<String>,
    pub(in crate::backend::direct_wasm) parameter_defaults: Vec<Option<Expression>>,
    pub(in crate::backend::direct_wasm) body_declares_arguments_binding: bool,
    pub(in crate::backend::direct_wasm) length: u32,
    pub(in crate::backend::direct_wasm) extra_argument_indices: Vec<u32>,
    pub(in crate::backend::direct_wasm) enumerated_keys_param_index: Option<usize>,
    pub(in crate::backend::direct_wasm) returns_arguments_object: bool,
    pub(in crate::backend::direct_wasm) returned_arguments_effects: ReturnedArgumentsEffects,
    pub(in crate::backend::direct_wasm) returned_member_function_bindings:
        Vec<ReturnedMemberFunctionBinding>,
    pub(in crate::backend::direct_wasm) returned_member_value_bindings:
        Vec<ReturnedMemberValueBinding>,
    pub(in crate::backend::direct_wasm) inline_summary: Option<InlineFunctionSummary>,
    pub(in crate::backend::direct_wasm) home_object_binding: Option<String>,
    pub(in crate::backend::direct_wasm) strict: bool,
    pub(in crate::backend::direct_wasm) lexical_this: bool,
    pub(in crate::backend::direct_wasm) function_index: u32,
    pub(in crate::backend::direct_wasm) type_index: u32,
}

impl UserFunction {
    pub(in crate::backend::direct_wasm) fn is_async(&self) -> bool {
        self.kind.is_async()
    }

    pub(in crate::backend::direct_wasm) fn is_arrow(&self) -> bool {
        self.lexical_this
    }

    pub(in crate::backend::direct_wasm) fn is_generator(&self) -> bool {
        self.kind.is_generator()
    }

    pub(in crate::backend::direct_wasm) fn is_constructible(&self) -> bool {
        matches!(self.kind, FunctionKind::Ordinary) && !self.lexical_this
    }

    pub(in crate::backend::direct_wasm) fn visible_param_count(&self) -> u32 {
        self.params.len() as u32
    }

    pub(in crate::backend::direct_wasm) fn has_parameter_defaults(&self) -> bool {
        self.parameter_defaults.iter().any(Option::is_some)
    }

    pub(in crate::backend::direct_wasm) fn wasm_param_count(&self) -> u32 {
        self.visible_param_count() + 1 + self.extra_argument_indices.len() as u32
    }

    pub(in crate::backend::direct_wasm) fn actual_argument_count_param(&self) -> u32 {
        self.visible_param_count()
    }

    pub(in crate::backend::direct_wasm) fn extra_argument_param(&self, index: u32) -> Option<u32> {
        let position = self
            .extra_argument_indices
            .iter()
            .position(|candidate| *candidate == index)? as u32;
        Some(self.visible_param_count() + 1 + position)
    }
}

pub(in crate::backend::direct_wasm) struct CompiledFunction {
    pub(in crate::backend::direct_wasm) local_count: u32,
    pub(in crate::backend::direct_wasm) instructions: Vec<u8>,
}

#[derive(Default)]
pub(in crate::backend::direct_wasm) struct ArgumentsUsage {
    pub(in crate::backend::direct_wasm) indexed_slots: Vec<u32>,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct ArgumentsIndexedPropertyState {
    pub(in crate::backend::direct_wasm) present: bool,
    pub(in crate::backend::direct_wasm) mapped: bool,
    pub(in crate::backend::direct_wasm) writable: bool,
    pub(in crate::backend::direct_wasm) enumerable: bool,
    pub(in crate::backend::direct_wasm) configurable: bool,
    pub(in crate::backend::direct_wasm) getter: Option<Expression>,
    pub(in crate::backend::direct_wasm) setter: Option<Expression>,
}

impl ArgumentsIndexedPropertyState {
    pub(in crate::backend::direct_wasm) fn data(present: bool, mapped: bool) -> Self {
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

    pub(in crate::backend::direct_wasm) fn is_accessor(&self) -> bool {
        self.getter.is_some() || self.setter.is_some()
    }
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct ArgumentsSlot {
    pub(in crate::backend::direct_wasm) value_local: u32,
    pub(in crate::backend::direct_wasm) present_local: u32,
    pub(in crate::backend::direct_wasm) mapped_local: Option<u32>,
    pub(in crate::backend::direct_wasm) source_param_local: Option<u32>,
    pub(in crate::backend::direct_wasm) state: ArgumentsIndexedPropertyState,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct ArgumentsValueBinding {
    pub(in crate::backend::direct_wasm) values: Vec<Expression>,
    pub(in crate::backend::direct_wasm) strict: bool,
    pub(in crate::backend::direct_wasm) callee_present: bool,
    pub(in crate::backend::direct_wasm) callee_value: Option<Expression>,
    pub(in crate::backend::direct_wasm) length_present: bool,
    pub(in crate::backend::direct_wasm) length_value: Expression,
}

#[derive(Clone, PartialEq)]
pub(in crate::backend::direct_wasm) struct ArrayValueBinding {
    pub(in crate::backend::direct_wasm) values: Vec<Option<Expression>>,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct RuntimeArraySlot {
    pub(in crate::backend::direct_wasm) value_local: u32,
    pub(in crate::backend::direct_wasm) present_local: u32,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct ResizableArrayBufferBinding {
    pub(in crate::backend::direct_wasm) values: Vec<Option<Expression>>,
    pub(in crate::backend::direct_wasm) max_length: usize,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct TypedArrayViewBinding {
    pub(in crate::backend::direct_wasm) buffer_name: String,
    pub(in crate::backend::direct_wasm) offset: usize,
    pub(in crate::backend::direct_wasm) fixed_length: Option<usize>,
}

#[derive(Clone, PartialEq)]
pub(in crate::backend::direct_wasm) struct ObjectValueBinding {
    pub(in crate::backend::direct_wasm) string_properties: Vec<(String, Expression)>,
    pub(in crate::backend::direct_wasm) symbol_properties: Vec<(Expression, Expression)>,
    pub(in crate::backend::direct_wasm) non_enumerable_string_properties: Vec<String>,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) enum IteratorSourceKind {
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
pub(in crate::backend::direct_wasm) struct ArrayIteratorBinding {
    pub(in crate::backend::direct_wasm) source: IteratorSourceKind,
    pub(in crate::backend::direct_wasm) index_local: u32,
    pub(in crate::backend::direct_wasm) static_index: Option<usize>,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct SimpleGeneratorStep {
    pub(in crate::backend::direct_wasm) effects: Vec<Statement>,
    pub(in crate::backend::direct_wasm) outcome: SimpleGeneratorStepOutcome,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) enum SimpleGeneratorStepOutcome {
    Yield(Expression),
    Throw(Expression),
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) enum IteratorStepBinding {
    Runtime {
        done_local: u32,
        value_local: u32,
        function_binding: Option<LocalFunctionBinding>,
        static_done: Option<bool>,
        static_value: Option<Expression>,
    },
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct ProxyValueBinding {
    pub(in crate::backend::direct_wasm) target: Expression,
    pub(in crate::backend::direct_wasm) has_binding: Option<LocalFunctionBinding>,
}

impl ArgumentsValueBinding {
    pub(in crate::backend::direct_wasm) fn for_user_function(
        user_function: &UserFunction,
        values: Vec<Expression>,
    ) -> Self {
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

    pub(in crate::backend::direct_wasm) fn apply_effects(
        &mut self,
        effects: &ReturnedArgumentsEffects,
    ) {
        if let Some(effect) = &effects.callee {
            self.apply_named_effect("callee", effect.clone());
        }
        if let Some(effect) = &effects.length {
            self.apply_named_effect("length", effect.clone());
        }
    }

    pub(in crate::backend::direct_wasm) fn apply_named_effect(
        &mut self,
        property_name: &str,
        effect: ArgumentsPropertyEffect,
    ) {
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
pub(in crate::backend::direct_wasm) struct ReturnedArgumentsEffects {
    pub(in crate::backend::direct_wasm) callee: Option<ArgumentsPropertyEffect>,
    pub(in crate::backend::direct_wasm) length: Option<ArgumentsPropertyEffect>,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) enum ArgumentsPropertyEffect {
    Assign(Expression),
    Delete,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct PropertyDescriptorBinding {
    pub(in crate::backend::direct_wasm) value: Option<Expression>,
    pub(in crate::backend::direct_wasm) configurable: bool,
    pub(in crate::backend::direct_wasm) enumerable: bool,
    pub(in crate::backend::direct_wasm) writable: Option<bool>,
    pub(in crate::backend::direct_wasm) has_get: bool,
    pub(in crate::backend::direct_wasm) has_set: bool,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct GlobalPropertyDescriptorState {
    pub(in crate::backend::direct_wasm) value: Expression,
    pub(in crate::backend::direct_wasm) writable: Option<bool>,
    pub(in crate::backend::direct_wasm) enumerable: bool,
    pub(in crate::backend::direct_wasm) configurable: bool,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) enum StringConcatFragment {
    Static(String),
    Dynamic(Expression),
}

#[derive(Clone, Default)]
pub(in crate::backend::direct_wasm) struct PropertyDescriptorDefinition {
    pub(in crate::backend::direct_wasm) value: Option<Expression>,
    pub(in crate::backend::direct_wasm) writable: Option<bool>,
    pub(in crate::backend::direct_wasm) enumerable: Option<bool>,
    pub(in crate::backend::direct_wasm) configurable: Option<bool>,
    pub(in crate::backend::direct_wasm) getter: Option<Expression>,
    pub(in crate::backend::direct_wasm) setter: Option<Expression>,
}

impl PropertyDescriptorDefinition {
    pub(in crate::backend::direct_wasm) fn is_accessor(&self) -> bool {
        self.getter.is_some() || self.setter.is_some()
    }
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct LoopContext {
    pub(in crate::backend::direct_wasm) break_target: usize,
    pub(in crate::backend::direct_wasm) continue_target: usize,
    pub(in crate::backend::direct_wasm) labels: Vec<String>,
    pub(in crate::backend::direct_wasm) assigned_bindings: HashSet<String>,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct BreakContext {
    pub(in crate::backend::direct_wasm) break_target: usize,
    pub(in crate::backend::direct_wasm) labels: Vec<String>,
    pub(in crate::backend::direct_wasm) break_hook: Option<Expression>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub(in crate::backend::direct_wasm) enum MemberFunctionBindingTarget {
    Identifier(String),
    Prototype(String),
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub(in crate::backend::direct_wasm) enum MemberFunctionBindingProperty {
    String(String),
    Symbol(String),
    SymbolExpression(String),
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub(in crate::backend::direct_wasm) struct MemberFunctionBindingKey {
    pub(in crate::backend::direct_wasm) target: MemberFunctionBindingTarget,
    pub(in crate::backend::direct_wasm) property: MemberFunctionBindingProperty,
}

pub(in crate::backend::direct_wasm) struct MaterializationGuard<'a> {
    pub(in crate::backend::direct_wasm) active: &'a RefCell<HashSet<usize>>,
    pub(in crate::backend::direct_wasm) key: usize,
}

impl Drop for MaterializationGuard<'_> {
    fn drop(&mut self) {
        self.active.borrow_mut().remove(&self.key);
    }
}

pub(in crate::backend::direct_wasm) struct FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) module: &'a mut DirectWasmCompiler,
    pub(in crate::backend::direct_wasm) state: FunctionCompilerState,
}

impl Deref for FunctionCompiler<'_> {
    type Target = FunctionCompilerState;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl DerefMut for FunctionCompiler<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.state
    }
}

pub(in crate::backend::direct_wasm) struct FunctionCompilerState {
    pub(in crate::backend::direct_wasm) parameter_names: Vec<String>,
    pub(in crate::backend::direct_wasm) parameter_defaults: Vec<Option<Expression>>,
    pub(in crate::backend::direct_wasm) parameter_initialized_locals: HashMap<String, u32>,
    pub(in crate::backend::direct_wasm) parameter_scope_arguments_local: Option<u32>,
    pub(in crate::backend::direct_wasm) in_parameter_default_initialization: bool,
    pub(in crate::backend::direct_wasm) locals: HashMap<String, u32>,
    pub(in crate::backend::direct_wasm) local_kinds: HashMap<String, StaticValueKind>,
    pub(in crate::backend::direct_wasm) local_value_bindings: HashMap<String, Expression>,
    pub(in crate::backend::direct_wasm) local_function_bindings:
        HashMap<String, LocalFunctionBinding>,
    pub(in crate::backend::direct_wasm) local_specialized_function_values:
        HashMap<String, SpecializedFunctionValue>,
    pub(in crate::backend::direct_wasm) local_proxy_bindings: HashMap<String, ProxyValueBinding>,
    pub(in crate::backend::direct_wasm) member_function_bindings:
        HashMap<MemberFunctionBindingKey, LocalFunctionBinding>,
    pub(in crate::backend::direct_wasm) member_function_capture_slots:
        HashMap<MemberFunctionBindingKey, BTreeMap<String, String>>,
    pub(in crate::backend::direct_wasm) member_getter_bindings:
        HashMap<MemberFunctionBindingKey, LocalFunctionBinding>,
    pub(in crate::backend::direct_wasm) member_setter_bindings:
        HashMap<MemberFunctionBindingKey, LocalFunctionBinding>,
    pub(in crate::backend::direct_wasm) local_array_bindings: HashMap<String, ArrayValueBinding>,
    pub(in crate::backend::direct_wasm) local_resizable_array_buffer_bindings:
        HashMap<String, ResizableArrayBufferBinding>,
    pub(in crate::backend::direct_wasm) local_typed_array_view_bindings:
        HashMap<String, TypedArrayViewBinding>,
    pub(in crate::backend::direct_wasm) runtime_typed_array_oob_locals: HashMap<String, u32>,
    pub(in crate::backend::direct_wasm) tracked_array_function_values:
        HashMap<String, HashMap<u32, SpecializedFunctionValue>>,
    pub(in crate::backend::direct_wasm) runtime_array_slots:
        HashMap<String, HashMap<u32, RuntimeArraySlot>>,
    pub(in crate::backend::direct_wasm) local_array_iterator_bindings:
        HashMap<String, ArrayIteratorBinding>,
    pub(in crate::backend::direct_wasm) local_iterator_step_bindings:
        HashMap<String, IteratorStepBinding>,
    pub(in crate::backend::direct_wasm) runtime_array_length_locals: HashMap<String, u32>,
    pub(in crate::backend::direct_wasm) materializing_expression_keys: RefCell<HashSet<usize>>,
    pub(in crate::backend::direct_wasm) local_object_bindings: HashMap<String, ObjectValueBinding>,
    pub(in crate::backend::direct_wasm) local_prototype_object_bindings:
        HashMap<String, ObjectValueBinding>,
    pub(in crate::backend::direct_wasm) local_arguments_bindings:
        HashMap<String, ArgumentsValueBinding>,
    pub(in crate::backend::direct_wasm) direct_arguments_aliases: HashSet<String>,
    pub(in crate::backend::direct_wasm) local_descriptor_bindings:
        HashMap<String, PropertyDescriptorBinding>,
    pub(in crate::backend::direct_wasm) eval_lexical_initialized_locals: HashMap<String, u32>,
    pub(in crate::backend::direct_wasm) throw_tag_local: u32,
    pub(in crate::backend::direct_wasm) throw_value_local: u32,
    pub(in crate::backend::direct_wasm) strict_mode: bool,
    pub(in crate::backend::direct_wasm) next_local_index: u32,
    pub(in crate::backend::direct_wasm) param_count: u32,
    pub(in crate::backend::direct_wasm) visible_param_count: u32,
    pub(in crate::backend::direct_wasm) actual_argument_count_local: Option<u32>,
    pub(in crate::backend::direct_wasm) extra_argument_param_locals: HashMap<u32, u32>,
    pub(in crate::backend::direct_wasm) arguments_slots: HashMap<u32, ArgumentsSlot>,
    pub(in crate::backend::direct_wasm) mapped_arguments: bool,
    pub(in crate::backend::direct_wasm) current_user_function_name: Option<String>,
    pub(in crate::backend::direct_wasm) current_arguments_callee_present: bool,
    pub(in crate::backend::direct_wasm) current_arguments_callee_override: Option<Expression>,
    pub(in crate::backend::direct_wasm) current_arguments_length_present: bool,
    pub(in crate::backend::direct_wasm) current_arguments_length_override: Option<Expression>,
    pub(in crate::backend::direct_wasm) capture_slot_source_bindings: HashMap<String, String>,
    pub(in crate::backend::direct_wasm) deleted_builtin_identifiers: HashSet<String>,
    pub(in crate::backend::direct_wasm) runtime_dynamic_bindings: HashSet<String>,
    pub(in crate::backend::direct_wasm) last_bound_user_function_call:
        Option<BoundUserFunctionCallSnapshot>,
    pub(in crate::backend::direct_wasm) instructions: Vec<u8>,
    pub(in crate::backend::direct_wasm) control_stack: Vec<()>,
    pub(in crate::backend::direct_wasm) loop_stack: Vec<LoopContext>,
    pub(in crate::backend::direct_wasm) break_stack: Vec<BreakContext>,
    pub(in crate::backend::direct_wasm) active_eval_lexical_scopes:
        Vec<Vec<(String, Option<String>)>>,
    pub(in crate::backend::direct_wasm) active_eval_lexical_binding_counts: HashMap<String, u32>,
    pub(in crate::backend::direct_wasm) active_scoped_lexical_bindings:
        HashMap<String, Vec<String>>,
    pub(in crate::backend::direct_wasm) with_scopes: Vec<Expression>,
    pub(in crate::backend::direct_wasm) try_stack: Vec<TryContext>,
    pub(in crate::backend::direct_wasm) allow_return: bool,
    pub(in crate::backend::direct_wasm) top_level_function: bool,
    pub(in crate::backend::direct_wasm) isolated_indirect_eval: bool,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct TryContext {
    pub(in crate::backend::direct_wasm) catch_target: usize,
}
