use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn global_has_binding(&self, name: &str) -> bool {
        self.backend.global_has_binding(name)
    }

    pub(in crate::backend::direct_wasm) fn global_has_implicit_binding(&self, name: &str) -> bool {
        self.backend.global_has_implicit_binding(name)
    }

    pub(in crate::backend::direct_wasm) fn global_binding_kind(
        &self,
        name: &str,
    ) -> Option<StaticValueKind> {
        self.backend.global_binding_kind(name)
    }

    pub(in crate::backend::direct_wasm) fn clear_global_binding_kind(&mut self, name: &str) {
        self.backend.clear_global_binding_kind(name);
    }

    pub(in crate::backend::direct_wasm) fn implicit_global_binding(
        &self,
        name: &str,
    ) -> Option<ImplicitGlobalBinding> {
        self.backend.implicit_global_binding(name)
    }

    pub(in crate::backend::direct_wasm) fn ensure_implicit_global_binding(
        &mut self,
        name: &str,
    ) -> ImplicitGlobalBinding {
        self.backend.ensure_implicit_global_binding(name)
    }

    pub(in crate::backend::direct_wasm) fn clear_global_binding_state(&mut self, name: &str) {
        self.backend.clear_global_binding_state(name);
    }

    pub(in crate::backend::direct_wasm) fn update_static_global_assignment_metadata(
        &mut self,
        name: &str,
        value: &Expression,
    ) {
        let snapshot_value = self
            .global_value_binding(name)
            .map(|snapshot| substitute_self_referential_binding_snapshot(value, name, snapshot))
            .unwrap_or_else(|| value.clone());
        let mut referenced_names = HashSet::new();
        collect_referenced_binding_names_from_expression(&snapshot_value, &mut referenced_names);
        if referenced_names.contains(name) {
            self.clear_global_binding_state(name);
            return;
        }

        let materialized_value =
            if let Some(bigint) = self.resolve_static_bigint_value(&snapshot_value) {
                Expression::BigInt(bigint.to_string())
            } else {
                self.resolve_static_string_value(&snapshot_value)
                    .map(Expression::String)
                    .unwrap_or_else(|| self.materialize_static_expression(&snapshot_value))
            };
        let kind = self
            .infer_value_kind(&snapshot_value)
            .unwrap_or(StaticValueKind::Unknown);

        self.backend.set_global_binding_kind(name, kind);
        self.backend
            .sync_global_expression_binding(name, Some(materialized_value));
        self.backend.sync_global_array_binding(
            name,
            self.resolve_array_binding_from_expression(&snapshot_value),
        );
        self.backend.sync_global_object_binding(
            name,
            self.resolve_object_binding_from_expression(&snapshot_value),
        );
        self.backend.sync_global_arguments_binding(
            name,
            self.resolve_arguments_binding_from_expression(&snapshot_value),
        );
        self.backend.sync_global_function_binding(
            name,
            self.resolve_function_binding_from_expression(&snapshot_value),
        );
    }

    pub(in crate::backend::direct_wasm) fn allocate_test262_realm(&mut self) -> u32 {
        self.backend.allocate_test262_realm()
    }

    pub(in crate::backend::direct_wasm) fn global_value_binding(
        &self,
        name: &str,
    ) -> Option<&Expression> {
        self.backend.global_value_binding(name)
    }

    pub(in crate::backend::direct_wasm) fn global_object_binding(
        &self,
        name: &str,
    ) -> Option<&ObjectValueBinding> {
        self.backend.global_object_binding(name)
    }

    pub(in crate::backend::direct_wasm) fn global_array_binding(
        &self,
        name: &str,
    ) -> Option<&ArrayValueBinding> {
        self.backend.global_array_binding(name)
    }

    pub(in crate::backend::direct_wasm) fn global_prototype_object_binding(
        &self,
        name: &str,
    ) -> Option<&ObjectValueBinding> {
        self.backend.global_prototype_object_binding(name)
    }

    pub(in crate::backend::direct_wasm) fn global_proxy_binding(
        &self,
        name: &str,
    ) -> Option<&ProxyValueBinding> {
        self.backend.global_proxy_binding(name)
    }

    pub(in crate::backend::direct_wasm) fn global_object_prototype_expression(
        &self,
        name: &str,
    ) -> Option<&Expression> {
        self.backend.global_object_prototype_expression(name)
    }

    pub(in crate::backend::direct_wasm) fn find_global_home_object_binding_name(
        &self,
        function_name: &str,
    ) -> Option<String> {
        self.backend
            .find_global_home_object_binding_name(function_name)
    }

    pub(in crate::backend::direct_wasm) fn global_runtime_prototype_binding(
        &self,
        name: &str,
    ) -> Option<&GlobalObjectRuntimePrototypeBinding> {
        self.backend.global_runtime_prototype_binding(name)
    }

    pub(in crate::backend::direct_wasm) fn test262_realm_global_object_binding(
        &self,
        realm_id: u32,
    ) -> Option<ObjectValueBinding> {
        self.backend.test262_realm_global_object_binding(realm_id)
    }

    pub(in crate::backend::direct_wasm) fn test262_realm_mut(
        &mut self,
        realm_id: u32,
    ) -> Option<&mut Test262Realm> {
        self.backend.test262_realm_mut(realm_id)
    }
}
