use super::super::super::super::*;
use super::super::GlobalStaticEvaluationEnvironment;

impl GlobalStaticEvaluationEnvironment {
    pub(in crate::backend::direct_wasm) fn contains_local_binding(&self, name: &str) -> bool {
        self.local_bindings.contains_key(name)
    }

    pub(in crate::backend::direct_wasm) fn from_snapshots(
        local_bindings: HashMap<String, Expression>,
        value_bindings: HashMap<String, Expression>,
        object_bindings: HashMap<String, ObjectValueBinding>,
    ) -> Self {
        Self {
            local_bindings,
            value_bindings,
            object_bindings,
        }
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

    pub(in crate::backend::direct_wasm) fn set_value_binding(
        &mut self,
        name: String,
        value: Expression,
    ) {
        self.value_bindings.insert(name, value);
    }

    pub(in crate::backend::direct_wasm) fn assign_binding_value(
        &mut self,
        name: String,
        value: Expression,
    ) -> Expression {
        if self.contains_local_binding(&name) {
            return self.set_local_binding(name, value);
        }

        self.set_value_binding(name.clone(), value);
        self.value_bindings
            .get(&name)
            .cloned()
            .expect("fresh global static binding must exist")
    }

    pub(in crate::backend::direct_wasm) fn object_binding_mut(
        &mut self,
        name: &str,
    ) -> Option<&mut ObjectValueBinding> {
        self.object_bindings.get_mut(name)
    }

    pub(in crate::backend::direct_wasm) fn clear_local_bindings(&mut self) {
        self.local_bindings.clear();
    }

    pub(in crate::backend::direct_wasm) fn sync_object_binding(
        &mut self,
        name: &str,
        binding: Option<ObjectValueBinding>,
    ) {
        if let Some(binding) = binding {
            self.object_bindings.insert(name.to_string(), binding);
        } else {
            self.object_bindings.remove(name);
        }
    }
}
