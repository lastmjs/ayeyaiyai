use super::*;
use crate::backend::direct_wasm::static_eval::StaticUserFunctionBindingExecutor;

pub(in crate::backend::direct_wasm) fn materialize_expression_in_binding_maps<
    Materializer,
    ResolveObjectBinding,
    PreserveMissing,
>(
    materializer: &Materializer,
    expression: &Expression,
    local_bindings: &HashMap<String, Expression>,
    value_bindings: &HashMap<String, Expression>,
    object_bindings: &HashMap<String, ObjectValueBinding>,
    resolve_object_binding: &ResolveObjectBinding,
    preserve_missing: &PreserveMissing,
) -> Option<Expression>
where
    Materializer: StaticIdentifierMaterializer + ?Sized,
    ResolveObjectBinding: Fn(
        &Expression,
        &HashMap<String, Expression>,
        &HashMap<String, Expression>,
        &HashMap<String, ObjectValueBinding>,
    ) -> Option<ObjectValueBinding>,
    PreserveMissing: Fn(&Expression, &Expression) -> bool,
{
    let environment = StaticBindingMapsView::new(local_bindings, value_bindings, object_bindings);
    materialize_stateful_expression_in_environment(
        materializer,
        expression,
        &environment,
        &|expression, environment| {
            resolve_object_binding(
                expression,
                environment.local_bindings,
                environment.value_bindings,
                environment.object_bindings,
            )
        },
        &|full_expression, object, property, environment, recurse| {
            materialize_missing_member_expression_with_policy(
                full_expression,
                object,
                property,
                environment,
                recurse,
                &|_full_expression, object, property, _environment| {
                    preserve_missing(object, property)
                },
            )
        },
        &|expression, environment, recurse| {
            materialize_structural_expression(expression, false, false, environment, recurse)
        },
    )
}

pub(in crate::backend::direct_wasm) fn resolve_stateful_object_binding_in_binding_maps<
    ResolveObjectBinding,
>(
    expression: &Expression,
    local_bindings: &HashMap<String, Expression>,
    value_bindings: &HashMap<String, Expression>,
    object_bindings: &HashMap<String, ObjectValueBinding>,
    resolve_object_binding: &ResolveObjectBinding,
) -> Option<ObjectValueBinding>
where
    ResolveObjectBinding: Fn(
        &Expression,
        &HashMap<String, Expression>,
        &HashMap<String, Expression>,
        &HashMap<String, ObjectValueBinding>,
    ) -> Option<ObjectValueBinding>,
{
    let mut environment =
        StaticBindingMapsView::new(local_bindings, value_bindings, object_bindings);
    resolve_stateful_object_binding_from_environment(
        expression,
        &mut environment,
        &|expression, environment| {
            resolve_object_binding(
                expression,
                environment.local_bindings,
                environment.value_bindings,
                environment.object_bindings,
            )
        },
    )
}

pub(in crate::backend::direct_wasm) fn resolve_bound_alias_expression_in_environment<
    Environment,
    BlocksResolution,
    IsDynamic,
    LookupBinding,
>(
    expression: &Expression,
    environment: &Environment,
    blocks_resolution: &BlocksResolution,
    is_dynamic: &IsDynamic,
    lookup_binding: &LookupBinding,
) -> Option<Expression>
where
    BlocksResolution: Fn(&str) -> bool,
    IsDynamic: Fn(&str) -> bool,
    LookupBinding: Fn(&str, &Environment) -> Option<Expression>,
{
    let mut current = expression.clone();
    let mut visited = HashSet::new();
    loop {
        let Expression::Identifier(name) = &current else {
            return Some(current);
        };
        if blocks_resolution(name) || is_dynamic(name) {
            return Some(current);
        }
        if !visited.insert(name.clone()) {
            return None;
        }
        if let Some(value) = lookup_binding(name, environment)
            .filter(|value| !matches!(value, Expression::Identifier(alias) if alias == name))
        {
            current = value;
            continue;
        }
        return Some(current);
    }
}

pub(in crate::backend::direct_wasm) fn execute_static_user_function_binding_in_global_maps<
    Executor,
>(
    executor: &Executor,
    binding: &LocalFunctionBinding,
    arguments: &[CallArgument],
    value_bindings: &mut HashMap<String, Expression>,
    object_bindings: &mut HashMap<String, ObjectValueBinding>,
    effect_mode: StaticFunctionEffectMode,
) -> Option<Expression>
where
    Executor:
        StaticUserFunctionBindingExecutor<Environment = GlobalStaticEvaluationEnvironment> + ?Sized,
{
    GlobalStaticEvaluationEnvironment::with_global_bindings(
        value_bindings,
        object_bindings,
        |environment| {
            execute_static_user_function_binding_in_environment(
                executor,
                binding,
                arguments,
                environment,
                effect_mode,
            )
        },
    )
}

pub(in crate::backend::direct_wasm) fn assign_static_member_binding_in_global_maps<Executor>(
    executor: &Executor,
    object: &Expression,
    property: Expression,
    value: Expression,
    local_bindings: &mut HashMap<String, Expression>,
    value_bindings: &mut HashMap<String, Expression>,
    object_bindings: &mut HashMap<String, ObjectValueBinding>,
) -> Option<Expression>
where
    Executor: StaticExpressionExecutor<Environment = GlobalStaticEvaluationEnvironment> + ?Sized,
{
    GlobalStaticEvaluationEnvironment::with_state_bindings(
        local_bindings,
        value_bindings,
        object_bindings,
        |environment| {
            executor.assign_member_binding_value(object, property, value.clone(), environment)?;
            Some(value)
        },
    )
}
