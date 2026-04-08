use super::super::super::super::*;
use crate::backend::direct_wasm::GlobalValueService;

impl GlobalValueService {
    pub(in crate::backend::direct_wasm) fn snapshot_value_bindings(
        &self,
    ) -> HashMap<String, Expression> {
        self.value_bindings.clone()
    }

    pub(in crate::backend::direct_wasm) fn snapshot_object_bindings(
        &self,
    ) -> HashMap<String, ObjectValueBinding> {
        self.object_bindings.clone()
    }

    pub(in crate::backend::direct_wasm) fn snapshot_top_level_static_state(
        &self,
    ) -> (
        HashMap<String, Expression>,
        HashMap<String, ObjectValueBinding>,
    ) {
        (
            self.snapshot_value_bindings(),
            self.snapshot_object_bindings(),
        )
    }
}
