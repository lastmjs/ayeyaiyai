use super::*;

#[derive(Default)]
pub(in crate::backend::direct_wasm) struct Test262State {
    pub(in crate::backend::direct_wasm) next_realm_id: u32,
    pub(in crate::backend::direct_wasm) realms: HashMap<u32, Test262Realm>,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct Test262Realm {
    pub(in crate::backend::direct_wasm) global_object_binding: ObjectValueBinding,
}
