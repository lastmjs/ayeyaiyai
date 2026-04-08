use super::super::super::super::*;
use super::super::super::StaticResolutionEnvironment;

impl StaticResolutionEnvironment {
    pub(in crate::backend::direct_wasm) fn clear_local_bindings(&mut self) {
        self.local_bindings.clear();
        self.local_object_bindings.clear();
        self.local_descriptor_bindings.clear();
    }

    pub(in crate::backend::direct_wasm) fn set_local_binding(
        &mut self,
        name: String,
        value: Expression,
    ) -> Expression {
        self.local_bindings.insert(name.clone(), value);
        self.local_bindings
            .get(&name)
            .cloned()
            .expect("fresh local static binding must exist")
    }
}
