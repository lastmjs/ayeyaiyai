use super::*;
mod branches;
mod consume_setup;
mod consume_step;
mod outcomes;
mod setup;
mod state;
mod step_result;
use self::setup::InitialDelegateSnapshotBindings;

pub(super) enum AsyncDelegateConsumptionPreparation {
    NotApplicable,
    Outcome(StaticEvalOutcome),
    Ready(PreparedAsyncDelegateConsumption),
}

pub(super) struct PreparedAsyncDelegateConsumption {
    pub(super) binding_name: String,
    pub(super) current_static_index: Option<usize>,
    pub(super) index_local: u32,
    pub(super) property_name: String,
    pub(super) plan: AsyncYieldDelegateGeneratorPlan,
    pub(super) delegate_iterator_name: String,
    pub(super) delegate_next_name: String,
    pub(super) delegate_completion_name: String,
    pub(super) delegate_iterator_expression: Expression,
    pub(super) delegate_completion_expression: Expression,
    pub(super) delegate_snapshot_bindings: Option<HashMap<String, Expression>>,
    pub(super) scoped_snapshot_names: Vec<String>,
    pub(super) snapshot_current_argument: Expression,
    pub(super) step_result_name: String,
    pub(super) promise_value_name: String,
    pub(super) promise_done_name: String,
}

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn consume_async_yield_delegate_generator_promise_outcome(
        &mut self,
        object: &Expression,
        property_name: &str,
        arguments: &[CallArgument],
    ) -> DirectResult<Option<StaticEvalOutcome>> {
        match self.prepare_async_yield_delegate_generator_consumption(
            object,
            property_name,
            arguments,
        )? {
            AsyncDelegateConsumptionPreparation::NotApplicable => Ok(None),
            AsyncDelegateConsumptionPreparation::Outcome(outcome) => Ok(Some(outcome)),
            AsyncDelegateConsumptionPreparation::Ready(prepared) => {
                self.consume_prepared_async_yield_delegate_generator_promise_outcome(prepared)
            }
        }
    }
}
