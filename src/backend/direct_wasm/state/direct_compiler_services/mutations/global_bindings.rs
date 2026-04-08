use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn ensure_global_binding_index(
        &mut self,
        name: &str,
        next_global_index: &mut u32,
    ) {
        self.state
            .ensure_global_binding_index(name, next_global_index);
    }

    pub(in crate::backend::direct_wasm) fn create_implicit_global_binding(
        &mut self,
        name: &str,
    ) -> ImplicitGlobalBinding {
        self.state.global_semantics.ensure_implicit_binding(name)
    }

    pub(in crate::backend::direct_wasm) fn set_global_binding_kind(
        &mut self,
        name: &str,
        kind: StaticValueKind,
    ) {
        self.state.set_global_binding_kind(name, kind);
    }

    pub(in crate::backend::direct_wasm) fn mark_global_lexical_binding(&mut self, name: &str) {
        self.state.mark_global_lexical_binding(name);
    }

    pub(in crate::backend::direct_wasm) fn set_global_expression_binding(
        &mut self,
        name: &str,
        value: Expression,
    ) {
        self.state.set_global_expression_binding(name, value);
    }

    pub(in crate::backend::direct_wasm) fn sync_global_array_binding(
        &mut self,
        name: &str,
        binding: Option<ArrayValueBinding>,
    ) {
        self.state.sync_global_array_binding(name, binding);
    }

    pub(in crate::backend::direct_wasm) fn sync_global_object_binding(
        &mut self,
        name: &str,
        binding: Option<ObjectValueBinding>,
    ) {
        self.state.sync_global_object_binding(name, binding);
    }

    pub(in crate::backend::direct_wasm) fn sync_global_arguments_binding(
        &mut self,
        name: &str,
        binding: Option<ArgumentsValueBinding>,
    ) {
        self.state.sync_global_arguments_binding(name, binding);
    }

    pub(in crate::backend::direct_wasm) fn sync_global_function_binding(
        &mut self,
        name: &str,
        binding: Option<LocalFunctionBinding>,
    ) {
        self.state.sync_global_function_binding(name, binding);
    }

    pub(in crate::backend::direct_wasm) fn upsert_global_data_property_descriptor(
        &mut self,
        name: &str,
        value: Expression,
        writable: Option<bool>,
        enumerable: bool,
        configurable: bool,
    ) {
        self.state.upsert_global_data_property_descriptor(
            name,
            value,
            writable,
            enumerable,
            configurable,
        );
    }

    pub(in crate::backend::direct_wasm) fn define_global_object_property(
        &mut self,
        name: &str,
        property: Expression,
        value: Expression,
        enumerable: bool,
    ) {
        self.state
            .define_global_object_property(name, property, value, enumerable);
    }

    pub(in crate::backend::direct_wasm) fn define_global_prototype_object_property(
        &mut self,
        name: &str,
        property: Expression,
        value: Expression,
        enumerable: bool,
    ) {
        self.state
            .define_global_prototype_object_property(name, property, value, enumerable);
    }

    pub(in crate::backend::direct_wasm) fn set_global_user_function_reference(
        &mut self,
        name: &str,
    ) {
        self.state.set_global_user_function_reference(name);
    }

    pub(in crate::backend::direct_wasm) fn set_global_array_element_binding(
        &mut self,
        name: &str,
        index: usize,
        value: Expression,
    ) -> bool {
        self.state
            .set_global_array_element_binding(name, index, value)
    }
}
