use super::*;

impl DirectWasmCompiler {
    #[cfg(test)]
    pub(in crate::backend::direct_wasm) fn infer_global_copy_data_properties_binding(
        &self,
        expression: &Expression,
    ) -> Option<ObjectValueBinding> {
        self.with_cloned_global_binding_state(|value_bindings, object_bindings| {
            let mut context = (value_bindings, object_bindings);
            resolve_copy_data_properties_binding(
                expression,
                &mut context,
                |expression, (value_bindings, object_bindings)| {
                    self.infer_global_object_binding_with_state(
                        expression,
                        value_bindings,
                        object_bindings,
                    )
                },
                |object, property, (value_bindings, object_bindings)| {
                    self.infer_global_member_getter_return_value_with_state(
                        object,
                        property,
                        value_bindings,
                        object_bindings,
                    )
                },
            )
        })
    }
}
