use super::*;
use std::sync::Arc;

mod binding_models;
mod compiler_global_binding_mutations;
mod compiler_global_binding_queries;
mod compiler_global_member_services;
mod compiler_global_value_mutations;
mod compiler_global_value_queries;
mod compiler_services;
mod direct_compiler_services;
mod function_backend_binding_services;
mod function_backend_core_services;
mod function_backend_member_services;
mod function_backend_test262_services;
mod function_backend_transaction_services;
mod function_backend_value_mutations;
mod function_backend_value_queries;
mod function_compiler_carriers;
mod function_models;
mod function_registry;
mod function_runtime;
mod function_semantics;
mod function_state;
mod function_state_binding_cleanup;
mod function_state_binding_snapshots;
mod function_state_eval_context;
mod function_state_init;
mod function_state_scopes;
mod global_binding_query_access;
mod global_member_access;
mod global_semantics;
mod global_value_query_access;
mod module_artifacts;
mod root_types;
mod static_env;
mod test262_state;
mod transactions;

pub(in crate::backend::direct_wasm) use binding_models::{
    ArgumentsIndexedPropertyState, ArgumentsPropertyEffect, ArgumentsSlot, ArgumentsUsage,
    ArgumentsValueBinding, ArrayIteratorBinding, ArrayValueBinding,
    AsyncYieldDelegateGeneratorPlan, BreakContext, CompiledFunction, GlobalPropertyDescriptorState,
    IteratorSourceKind, IteratorStepBinding, LoopContext, MaterializationGuard,
    MemberFunctionBindingKey, MemberFunctionBindingProperty, MemberFunctionBindingTarget,
    ObjectValueBinding, PropertyDescriptorBinding, PropertyDescriptorDefinition, ProxyValueBinding,
    ResizableArrayBufferBinding, ReturnedArgumentsEffects, RuntimeArraySlot, SimpleGeneratorStep,
    SimpleGeneratorStepOutcome, StringConcatFragment, TryContext, TypedArrayViewBinding,
};
pub(in crate::backend::direct_wasm) use function_compiler_carriers::{
    FunctionCompilationRequest, FunctionCompiler, FunctionCompilerBackend,
    FunctionCompilerBehavior, FunctionParameterBindingView, PreparedFunctionEntryState,
    PreparedFunctionExecutionContext, PreparedFunctionParameterState, PreparedFunctionRuntimeState,
    PreparedLocalStaticBindings,
};
pub(in crate::backend::direct_wasm) use function_models::{
    BoundUserFunctionCallSnapshot, InlineFunctionEffect, InlineFunctionSummary,
    LocalFunctionBinding, OrdinaryToPrimitiveAnalysis, OrdinaryToPrimitivePlan,
    OrdinaryToPrimitiveStep, PreparedBoundCaptureBinding, PreparedCaptureBinding, PrimitiveHint,
    ResolvedPropertyKey, ReturnedMemberFunctionBinding, ReturnedMemberFunctionBindingKey,
    ReturnedMemberFunctionBindingTarget, ReturnedMemberValueBinding, SpecializedFunctionValue,
    StaticEvalOutcome, StaticThrowValue, StaticValueKind, SymbolToPrimitiveHandling, UserFunction,
};
pub(in crate::backend::direct_wasm) use function_registry::{
    FunctionRegistryState, UserFunctionParameterAnalysis,
};
pub(in crate::backend::direct_wasm) use function_runtime::{
    FunctionRuntimeLocalsState, FunctionRuntimeState,
};
pub(in crate::backend::direct_wasm) use function_semantics::{
    FunctionEmissionState, FunctionExecutionContextState, FunctionLexicalScopeState,
    FunctionSpeculationState, FunctionStaticSemanticsState,
};
pub(in crate::backend::direct_wasm) use function_state::{
    FunctionCompilerState, FunctionParameterState,
};
pub(in crate::backend::direct_wasm) use global_binding_query_access::{
    GlobalBindingIndexQueryAccess, GlobalBindingKindQueryAccess, GlobalBindingPresenceQueryAccess,
    GlobalFunctionBindingQueryAccess, GlobalImplicitBindingQueryAccess,
};
pub(in crate::backend::direct_wasm) use global_member_access::{
    GlobalMemberAccessorMutationAccess, GlobalMemberAccessorQueryAccess,
    GlobalMemberBindingClearAccess, GlobalMemberCaptureMutationAccess,
    GlobalMemberCaptureQueryAccess, GlobalMemberFunctionMutationAccess,
    GlobalMemberFunctionQueryAccess,
};
pub(in crate::backend::direct_wasm) use global_semantics::{
    GlobalFunctionService, GlobalMemberService, GlobalNameService,
    GlobalObjectRuntimePrototypeBinding, GlobalSemanticState, GlobalValueService,
};
pub(in crate::backend::direct_wasm) use global_value_query_access::{
    GlobalArgumentsValueQueryAccess, GlobalArrayValueQueryAccess, GlobalIdentifierValueQueryAccess,
    GlobalObjectValueQueryAccess, GlobalPropertyDescriptorQueryAccess,
    GlobalRuntimePrototypeQueryAccess, GlobalValueBindingQueryAccess,
};
pub(in crate::backend::direct_wasm) use module_artifacts::ModuleArtifactsState;
pub(in crate::backend::direct_wasm) use root_types::{
    CompilerState, DirectWasmCompiler, ImplicitGlobalBinding,
};
pub(in crate::backend::direct_wasm) use static_env::{
    GlobalBindingEnvironment, GlobalStaticEvaluationEnvironment, SharedGlobalBindingEnvironment,
    StaticResolutionEnvironment,
};
pub(in crate::backend::direct_wasm) use test262_state::{Test262Realm, Test262State};
pub(in crate::backend::direct_wasm) use transactions::{
    FunctionStaticBindingMetadataSnapshot, FunctionStaticBindingMetadataTransaction,
    GlobalStaticSemanticsSnapshot, GlobalStaticSemanticsTransaction,
    IsolatedIndirectEvalTransaction, LocalStaticBindingSnapshot, LocalStaticBindingState,
    StaticBindingMetadataTransaction, UserFunctionExecutionContextSnapshot,
};
