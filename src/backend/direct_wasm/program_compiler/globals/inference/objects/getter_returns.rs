use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn infer_global_member_getter_return_value_with_state(
        &self,
        object: &Expression,
        property: &Expression,
        value_bindings: &mut HashMap<String, Expression>,
        object_bindings: &mut HashMap<String, ObjectValueBinding>,
    ) -> Option<Expression> {
        let getter_binding = self.infer_global_member_getter_binding(object, property)?;
        let context = self.static_eval_context();
        execute_static_user_function_binding_in_global_maps(
            &context,
            &getter_binding,
            &[],
            value_bindings,
            object_bindings,
            StaticFunctionEffectMode::Commit,
        )
    }

    #[cfg(test)]
    pub(in crate::backend::direct_wasm) fn infer_global_member_getter_return_value(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> Option<Expression> {
        self.with_cloned_global_binding_state(|value_bindings, object_bindings| {
            self.infer_global_member_getter_return_value_with_state(
                object,
                property,
                value_bindings,
                object_bindings,
            )
        })
    }
}
