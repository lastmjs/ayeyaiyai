use super::*;

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct FunctionRuntimeLocalsState {
    pub(in crate::backend::direct_wasm) bindings: HashMap<String, u32>,
    pub(in crate::backend::direct_wasm) next_local_index: u32,
    pub(in crate::backend::direct_wasm) deleted_builtin_identifiers: HashSet<String>,
    pub(in crate::backend::direct_wasm) runtime_dynamic_bindings: HashSet<String>,
}

impl FunctionRuntimeLocalsState {
    pub(in crate::backend::direct_wasm) fn from_prepared_state(
        prepared: &PreparedFunctionRuntimeState,
    ) -> Self {
        Self {
            bindings: prepared.locals.clone(),
            next_local_index: prepared.next_local_index,
            deleted_builtin_identifiers: HashSet::new(),
            runtime_dynamic_bindings: HashSet::new(),
        }
    }

    pub(in crate::backend::direct_wasm) fn contains_key(&self, name: &str) -> bool {
        self.bindings.contains_key(name)
    }

    pub(in crate::backend::direct_wasm) fn get(&self, name: &str) -> Option<&u32> {
        self.bindings.get(name)
    }

    pub(in crate::backend::direct_wasm) fn insert(
        &mut self,
        name: String,
        index: u32,
    ) -> Option<u32> {
        self.bindings.insert(name, index)
    }

    pub(in crate::backend::direct_wasm) fn remove(&mut self, name: &str) -> Option<u32> {
        self.bindings.remove(name)
    }

    pub(in crate::backend::direct_wasm) fn clear(&mut self) {
        self.bindings.clear();
    }

    pub(in crate::backend::direct_wasm) fn insert_runtime_dynamic_binding(&mut self, name: String) {
        self.runtime_dynamic_bindings.insert(name);
    }

    pub(in crate::backend::direct_wasm) fn contains_runtime_dynamic_binding(
        &self,
        name: &str,
    ) -> bool {
        self.runtime_dynamic_bindings.contains(name)
    }

    pub(in crate::backend::direct_wasm) fn remove_runtime_dynamic_binding(
        &mut self,
        name: &str,
    ) -> bool {
        self.runtime_dynamic_bindings.remove(name)
    }

    pub(in crate::backend::direct_wasm) fn iter(&self) -> impl Iterator<Item = (&String, &u32)> {
        self.bindings.iter()
    }

    pub(in crate::backend::direct_wasm) fn keys(&self) -> impl Iterator<Item = &String> {
        self.bindings.keys()
    }
}
