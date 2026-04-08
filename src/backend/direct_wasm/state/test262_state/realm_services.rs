use super::*;

impl Test262State {
    pub(in crate::backend::direct_wasm) fn reset_for_program(&mut self) {
        self.next_realm_id = 0;
        self.realms.clear();
    }

    pub(in crate::backend::direct_wasm) fn allocate_realm(&mut self) -> u32 {
        let realm_id = self.next_realm_id;
        self.next_realm_id += 1;
        self.realms.insert(
            realm_id,
            Test262Realm {
                global_object_binding: empty_object_value_binding(),
            },
        );
        realm_id
    }

    pub(in crate::backend::direct_wasm) fn has_realm(&self, realm_id: u32) -> bool {
        self.realms.contains_key(&realm_id)
    }

    pub(in crate::backend::direct_wasm) fn realm(&self, realm_id: u32) -> Option<&Test262Realm> {
        self.realms.get(&realm_id)
    }

    pub(in crate::backend::direct_wasm) fn realm_mut(
        &mut self,
        realm_id: u32,
    ) -> Option<&mut Test262Realm> {
        self.realms.get_mut(&realm_id)
    }

    pub(in crate::backend::direct_wasm) fn realm_global_object_binding(
        &self,
        realm_id: u32,
    ) -> Option<ObjectValueBinding> {
        self.realm(realm_id)
            .map(|realm| realm.global_object_binding.clone())
    }

    pub(in crate::backend::direct_wasm) fn set_realm_global_object_binding(
        &mut self,
        realm_id: u32,
        global_object_binding: ObjectValueBinding,
    ) {
        if let Some(realm) = self.realm_mut(realm_id) {
            realm.global_object_binding = global_object_binding;
        }
    }
}
