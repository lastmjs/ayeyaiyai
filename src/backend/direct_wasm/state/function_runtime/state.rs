use super::*;

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct FunctionRuntimeThrowState {
    pub(in crate::backend::direct_wasm) throw_tag_local: u32,
    pub(in crate::backend::direct_wasm) throw_value_local: u32,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct FunctionRuntimeBehaviorState {
    pub(in crate::backend::direct_wasm) allow_return: bool,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct FunctionRuntimeState {
    pub(in crate::backend::direct_wasm) locals: FunctionRuntimeLocalsState,
    pub(in crate::backend::direct_wasm) throws: FunctionRuntimeThrowState,
    pub(in crate::backend::direct_wasm) behavior: FunctionRuntimeBehaviorState,
}

impl FunctionRuntimeState {
    pub(in crate::backend::direct_wasm) fn from_prepared_state(
        prepared: PreparedFunctionRuntimeState,
    ) -> Self {
        Self {
            locals: FunctionRuntimeLocalsState::from_prepared_state(&prepared),
            throws: FunctionRuntimeThrowState {
                throw_tag_local: prepared.throw_tag_local,
                throw_value_local: prepared.throw_value_local,
            },
            behavior: FunctionRuntimeBehaviorState {
                allow_return: prepared.allow_return,
            },
        }
    }

    pub(in crate::backend::direct_wasm) fn clear_isolated_indirect_eval_state(&mut self) {
        self.locals.clear();
    }
}
