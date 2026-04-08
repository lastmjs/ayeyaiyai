use super::super::*;

impl CompilerState {
    pub(in crate::backend::direct_wasm) fn upsert_global_property_descriptor(
        &mut self,
        name: String,
        state: GlobalPropertyDescriptorState,
    ) {
        self.global_semantics
            .values
            .upsert_property_descriptor(name, state);
    }

    pub(in crate::backend::direct_wasm) fn upsert_global_data_property_descriptor(
        &mut self,
        name: &str,
        value: Expression,
        writable: Option<bool>,
        enumerable: bool,
        configurable: bool,
    ) {
        let mut descriptor = self.global_property_descriptor(name).cloned().unwrap_or(
            GlobalPropertyDescriptorState {
                value: Expression::Undefined,
                writable,
                enumerable,
                configurable,
            },
        );
        descriptor.value = value;
        descriptor.writable = writable;
        descriptor.enumerable = enumerable;
        descriptor.configurable = configurable;
        self.upsert_global_property_descriptor(name.to_string(), descriptor);
    }

    pub(in crate::backend::direct_wasm) fn define_global_object_property(
        &mut self,
        name: &str,
        property: Expression,
        value: Expression,
        enumerable: bool,
    ) {
        let mut binding = self
            .global_semantics
            .values
            .object_binding(name)
            .cloned()
            .unwrap_or_else(empty_object_value_binding);
        object_binding_define_property(&mut binding, property, value, enumerable);
        self.sync_global_object_binding(name, Some(binding));
    }

    pub(in crate::backend::direct_wasm) fn define_global_prototype_object_property(
        &mut self,
        name: &str,
        property: Expression,
        value: Expression,
        enumerable: bool,
    ) {
        let mut binding = self
            .global_semantics
            .values
            .prototype_object_binding(name)
            .cloned()
            .unwrap_or_else(empty_object_value_binding);
        object_binding_define_property(&mut binding, property, value, enumerable);
        self.sync_global_prototype_object_binding(name, Some(binding));
    }
}
