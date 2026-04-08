use super::*;
mod method_selection;
mod snapshot_bindings;

pub(in crate::backend::direct_wasm) enum InitialDelegateSnapshotBindings {
    Ready {
        bindings: HashMap<String, Expression>,
    },
    Throw {
        throw_value: StaticThrowValue,
        bindings: HashMap<String, Expression>,
    },
}
