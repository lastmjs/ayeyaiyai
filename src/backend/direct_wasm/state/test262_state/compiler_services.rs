use super::*;

impl CompilerState {
    pub(in crate::backend::direct_wasm) fn allocate_test262_realm(&mut self) -> u32 {
        let realm_id = self.test262.allocate_realm();

        let eval_builtin_name = test262_realm_eval_builtin_name(realm_id);
        let mut global_object_binding = empty_object_value_binding();
        object_binding_set_property(
            &mut global_object_binding,
            Expression::String("eval".to_string()),
            Expression::Identifier(eval_builtin_name),
        );

        self.test262
            .set_realm_global_object_binding(realm_id, global_object_binding);
        realm_id
    }

    pub(in crate::backend::direct_wasm) fn test262_realm_object_binding(
        &self,
        realm_id: u32,
    ) -> Option<ObjectValueBinding> {
        self.test262.has_realm(realm_id).then_some(())?;
        let mut realm_object_binding = empty_object_value_binding();
        object_binding_set_property(
            &mut realm_object_binding,
            Expression::String("global".to_string()),
            Expression::Identifier(test262_realm_global_identifier(realm_id)),
        );
        Some(realm_object_binding)
    }
}
