use super::super::super::*;

pub(in crate::backend::direct_wasm) struct ProgramStaticEvalContext<'a> {
    compiler: &'a DirectWasmCompiler,
}

impl<'a> ProgramStaticEvalContext<'a> {
    pub(in crate::backend::direct_wasm) fn new(compiler: &'a DirectWasmCompiler) -> Self {
        Self { compiler }
    }

    pub(in crate::backend::direct_wasm) fn infer_array_binding(
        &self,
        expression: &Expression,
    ) -> Option<ArrayValueBinding> {
        self.compiler.infer_global_array_binding(expression)
    }

    pub(in crate::backend::direct_wasm) fn infer_object_binding(
        &self,
        expression: &Expression,
    ) -> Option<ObjectValueBinding> {
        self.compiler.infer_global_object_binding(expression)
    }

    pub(in crate::backend::direct_wasm) fn infer_object_binding_with_state(
        &self,
        expression: &Expression,
        environment: &mut GlobalStaticEvaluationEnvironment,
    ) -> Option<ObjectValueBinding> {
        self.compiler.infer_global_object_binding_with_state(
            expression,
            &mut environment.value_bindings,
            &mut environment.object_bindings,
        )
    }

    pub(in crate::backend::direct_wasm) fn has_function_binding(
        &self,
        expression: &Expression,
    ) -> bool {
        self.compiler
            .infer_global_function_binding(expression)
            .is_some()
    }

    pub(in crate::backend::direct_wasm) fn has_prototype_object_binding(&self, name: &str) -> bool {
        self.compiler.global_has_prototype_object_binding(name)
    }

    pub(in crate::backend::direct_wasm) fn binding_kind(
        &self,
        name: &str,
    ) -> Option<StaticValueKind> {
        self.compiler.global_binding_kind(name)
    }

    pub(in crate::backend::direct_wasm) fn has_binding(&self, name: &str) -> bool {
        self.compiler.global_has_binding(name)
    }

    pub(in crate::backend::direct_wasm) fn has_lexical_binding(&self, name: &str) -> bool {
        self.compiler.global_has_lexical_binding(name)
    }

    pub(in crate::backend::direct_wasm) fn registered_function(
        &self,
        function_name: &str,
    ) -> Option<&FunctionDeclaration> {
        self.compiler.registered_function(function_name)
    }

    pub(in crate::backend::direct_wasm) fn user_function(
        &self,
        function_name: &str,
    ) -> Option<&UserFunction> {
        self.compiler.user_function(function_name)
    }

    pub(in crate::backend::direct_wasm) fn substitute_user_function_arguments(
        &self,
        expression: &Expression,
        user_function: &UserFunction,
        arguments: &[CallArgument],
    ) -> Expression {
        self.compiler
            .substitute_global_user_function_argument_bindings(expression, user_function, arguments)
    }

    pub(in crate::backend::direct_wasm) fn preserves_missing_member_function_capture(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> bool {
        preserves_missing_member_function_capture(
            object,
            property,
            |object, property| {
                self.compiler
                    .global_member_function_binding_key(object, property)
            },
            |key| self.compiler.has_global_member_function_capture_slots(key),
        )
    }
}
