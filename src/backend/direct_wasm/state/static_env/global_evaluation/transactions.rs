use super::super::super::super::*;
use super::super::GlobalStaticEvaluationEnvironment;

impl GlobalStaticEvaluationEnvironment {
    pub(in crate::backend::direct_wasm) fn with_state_bindings<T>(
        local_bindings: &mut HashMap<String, Expression>,
        value_bindings: &mut HashMap<String, Expression>,
        object_bindings: &mut HashMap<String, ObjectValueBinding>,
        callback: impl FnOnce(&mut GlobalStaticEvaluationEnvironment) -> T,
    ) -> T {
        let mut environment = GlobalStaticEvaluationEnvironment::from_snapshots(
            std::mem::take(local_bindings),
            std::mem::take(value_bindings),
            std::mem::take(object_bindings),
        );
        let result = callback(&mut environment);
        *local_bindings = environment.local_bindings;
        *value_bindings = environment.value_bindings;
        *object_bindings = environment.object_bindings;
        result
    }

    pub(in crate::backend::direct_wasm) fn with_global_bindings<T>(
        value_bindings: &mut HashMap<String, Expression>,
        object_bindings: &mut HashMap<String, ObjectValueBinding>,
        callback: impl FnOnce(&mut GlobalStaticEvaluationEnvironment) -> T,
    ) -> T {
        let mut local_bindings = HashMap::new();
        Self::with_state_bindings(
            &mut local_bindings,
            value_bindings,
            object_bindings,
            callback,
        )
    }
}
