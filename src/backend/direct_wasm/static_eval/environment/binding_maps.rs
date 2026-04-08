use super::*;

pub(in crate::backend::direct_wasm) struct StaticBindingMapsView<'a> {
    pub(in crate::backend::direct_wasm) local_bindings: &'a HashMap<String, Expression>,
    pub(in crate::backend::direct_wasm) value_bindings: &'a HashMap<String, Expression>,
    pub(in crate::backend::direct_wasm) object_bindings: &'a HashMap<String, ObjectValueBinding>,
}

impl<'a> StaticBindingMapsView<'a> {
    pub(in crate::backend::direct_wasm) fn new(
        local_bindings: &'a HashMap<String, Expression>,
        value_bindings: &'a HashMap<String, Expression>,
        object_bindings: &'a HashMap<String, ObjectValueBinding>,
    ) -> Self {
        Self {
            local_bindings,
            value_bindings,
            object_bindings,
        }
    }
}

impl StaticObjectBindingLookupEnvironment for StaticBindingMapsView<'_> {
    fn binding(&self, name: &str) -> Option<&Expression> {
        self.local_bindings
            .get(name)
            .or_else(|| self.value_bindings.get(name))
    }

    fn contains_object_binding(&self, name: &str) -> bool {
        self.object_bindings.contains_key(name)
    }
}

impl StaticObjectBindingEnvironment for StaticBindingMapsView<'_> {
    fn object_binding(&self, name: &str) -> Option<&ObjectValueBinding> {
        self.object_bindings.get(name)
    }
}

impl StaticIdentifierBindingEnvironment for StaticBindingMapsView<'_> {
    fn local_binding(&self, name: &str) -> Option<&Expression> {
        self.local_bindings.get(name)
    }

    fn global_value_binding(&self, name: &str) -> Option<&Expression> {
        self.value_bindings.get(name)
    }
}
