use super::super::super::super::*;
use super::super::super::StaticResolutionEnvironment;

impl StaticResolutionEnvironment {
    pub(in crate::backend::direct_wasm) fn set_value_binding(
        &mut self,
        name: String,
        value: Expression,
    ) -> Expression {
        if self.local_bindings.contains_key(&name) {
            return self.set_local_binding(name, value);
        }
        self.global_value_overrides.insert(name.clone(), value);
        self.global_value_overrides
            .get(&name)
            .cloned()
            .expect("fresh global static binding must exist")
    }

    pub(in crate::backend::direct_wasm) fn assign_binding_value(
        &mut self,
        name: String,
        value: Expression,
    ) -> Expression {
        self.set_value_binding(name, value)
    }
}
