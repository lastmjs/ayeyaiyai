use super::*;

pub(in crate::backend::direct_wasm) struct UserTypeRegistry {
    pub(in crate::backend::direct_wasm) next_user_type_index: u32,
    pub(in crate::backend::direct_wasm) user_type_indices: HashMap<u32, u32>,
    pub(in crate::backend::direct_wasm) user_type_arities: Vec<u32>,
}

impl Default for UserTypeRegistry {
    fn default() -> Self {
        Self {
            next_user_type_index: USER_TYPE_BASE_INDEX,
            user_type_indices: HashMap::new(),
            user_type_arities: Vec::new(),
        }
    }
}

impl UserTypeRegistry {
    pub(in crate::backend::direct_wasm) fn reset_for_program(&mut self) {
        self.next_user_type_index = USER_TYPE_BASE_INDEX;
        self.user_type_indices.clear();
        self.user_type_arities.clear();
    }

    pub(in crate::backend::direct_wasm) fn type_index_for_arity(&mut self, arity: u32) -> u32 {
        if let Some(type_index) = self.user_type_indices.get(&arity) {
            return *type_index;
        }
        let type_index = self.next_user_type_index;
        self.next_user_type_index += 1;
        self.user_type_indices.insert(arity, type_index);
        self.user_type_arities.push(arity);
        type_index
    }

    pub(in crate::backend::direct_wasm) fn user_type_arities_snapshot(&self) -> Vec<u32> {
        self.user_type_arities.clone()
    }
}
