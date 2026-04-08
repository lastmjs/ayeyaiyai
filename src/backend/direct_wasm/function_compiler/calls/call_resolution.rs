use super::*;

const SNAPSHOT_AWAIT_RESOLVE_BINDING: &str = "__ayy_snapshot_await_resolve";
const SNAPSHOT_AWAIT_REJECT_BINDING: &str = "__ayy_snapshot_await_reject";
const SNAPSHOT_AWAIT_RESOLUTION_VALUE: &str = "__ayy_snapshot_await_resolution";
const SNAPSHOT_AWAIT_REJECTION_VALUE: &str = "__ayy_snapshot_await_rejection";

thread_local! {
    static ACTIVE_BOUND_SNAPSHOT_EXPRESSIONS: std::cell::RefCell<std::collections::HashSet<String>> =
        std::cell::RefCell::new(std::collections::HashSet::new());
}

struct BoundSnapshotExpressionGuard {
    key: String,
}

impl BoundSnapshotExpressionGuard {
    fn enter(expression: &Expression, current_function_name: Option<&str>) -> Option<Self> {
        let key = format!("{current_function_name:?}:{expression:?}");
        ACTIVE_BOUND_SNAPSHOT_EXPRESSIONS.with(|active| {
            let mut active = active.borrow_mut();
            if !active.insert(key.clone()) {
                return None;
            }
            Some(Self { key })
        })
    }
}

impl Drop for BoundSnapshotExpressionGuard {
    fn drop(&mut self) {
        ACTIVE_BOUND_SNAPSHOT_EXPRESSIONS.with(|active| {
            active.borrow_mut().remove(&self.key);
        });
    }
}

pub(in crate::backend::direct_wasm) enum BoundSnapshotControlFlow {
    None,
    Return(Expression),
    Throw(Expression),
}

pub(in crate::backend::direct_wasm) struct PreparedStaticUserFunctionExecution {
    pub(in crate::backend::direct_wasm) substituted_body: Vec<Statement>,
    pub(in crate::backend::direct_wasm) environment: StaticResolutionEnvironment,
}

mod bound_snapshots;
#[path = "call_resolution/inline_effect_emission.rs"]
mod inline_effect_emission;
mod inline_summaries;
mod returned_values;
mod runtime_scans;
#[path = "call_resolution/statement_substitution.rs"]
mod statement_substitution;
mod static_user_functions;
mod substitutions;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_bound_snapshot_binding_name<'b>(
        &self,
        name: &'b str,
        bindings: &HashMap<String, Expression>,
    ) -> &'b str {
        if bindings.contains_key(name) {
            return name;
        }
        scoped_binding_source_name(name)
            .filter(|source_name| bindings.contains_key(*source_name))
            .unwrap_or(name)
    }
}
