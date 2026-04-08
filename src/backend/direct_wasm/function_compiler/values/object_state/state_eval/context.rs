use super::super::super::*;

pub(in crate::backend::direct_wasm) struct FunctionStaticEvalContext<'b, 'a> {
    compiler: &'b FunctionCompiler<'a>,
}

impl<'b, 'a> FunctionStaticEvalContext<'b, 'a> {
    pub(in crate::backend::direct_wasm) fn new(compiler: &'b FunctionCompiler<'a>) -> Self {
        Self { compiler }
    }

    pub(in crate::backend::direct_wasm) fn resolve_array_binding(
        &self,
        expression: &Expression,
    ) -> Option<ArrayValueBinding> {
        self.compiler
            .resolve_array_binding_from_expression(expression)
    }

    pub(in crate::backend::direct_wasm) fn resolve_object_binding(
        &self,
        expression: &Expression,
    ) -> Option<ObjectValueBinding> {
        self.compiler
            .resolve_object_binding_from_expression(expression)
    }

    pub(in crate::backend::direct_wasm) fn resolve_function_binding(
        &self,
        expression: &Expression,
    ) -> Option<LocalFunctionBinding> {
        self.compiler
            .resolve_function_binding_from_expression(expression)
    }

    pub(in crate::backend::direct_wasm) fn has_local_prototype_object_binding(
        &self,
        name: &str,
    ) -> bool {
        self.compiler
            .state
            .speculation
            .static_semantics
            .objects
            .local_prototype_object_bindings
            .contains_key(name)
    }

    pub(in crate::backend::direct_wasm) fn has_global_prototype_object_binding(
        &self,
        name: &str,
    ) -> bool {
        self.compiler
            .global_prototype_object_binding(name)
            .is_some()
    }

    pub(in crate::backend::direct_wasm) fn resolve_property_key(
        &self,
        property: &Expression,
    ) -> Option<Expression> {
        self.compiler.resolve_property_key_expression(property)
    }

    pub(in crate::backend::direct_wasm) fn lookup_identifier_kind(
        &self,
        name: &str,
    ) -> Option<StaticValueKind> {
        self.compiler.lookup_identifier_kind(name)
    }

    pub(in crate::backend::direct_wasm) fn is_unshadowed_builtin_identifier(
        &self,
        name: &str,
    ) -> bool {
        FunctionCompiler::is_unshadowed_builtin_identifier(self.compiler, name)
    }

    pub(in crate::backend::direct_wasm) fn materialize_expression(
        &self,
        expression: &Expression,
    ) -> Expression {
        self.compiler.materialize_static_expression(expression)
    }

    pub(in crate::backend::direct_wasm) fn evaluate_expression_with_state(
        &self,
        expression: &Expression,
        environment: &mut StaticResolutionEnvironment,
    ) -> Option<Expression> {
        self.evaluate_static_expression_with_state(expression, environment)
    }

    pub(in crate::backend::direct_wasm) fn user_function(
        &self,
        function_name: &str,
    ) -> Option<&UserFunction> {
        self.compiler.user_function(function_name)
    }

    pub(in crate::backend::direct_wasm) fn registered_function_declaration(
        &self,
        function_name: &str,
    ) -> Option<&FunctionDeclaration> {
        self.compiler
            .resolve_registered_function_declaration(function_name)
    }

    pub(in crate::backend::direct_wasm) fn substitute_user_function_arguments(
        &self,
        expression: &Expression,
        user_function: &UserFunction,
        arguments: &[CallArgument],
    ) -> Expression {
        self.compiler.substitute_user_function_argument_bindings(
            expression,
            user_function,
            arguments,
        )
    }

    pub(in crate::backend::direct_wasm) fn resolve_object_binding_with_state(
        &self,
        binding_expression: &Expression,
        environment: &mut StaticResolutionEnvironment,
    ) -> Option<ObjectValueBinding> {
        self.compiler
            .resolve_object_binding_from_expression_with_state(binding_expression, environment)
    }

    pub(in crate::backend::direct_wasm) fn resolve_constructor_capture_source_bindings(
        &self,
        callee: &Expression,
    ) -> Option<HashMap<String, Expression>> {
        self.compiler
            .resolve_constructor_capture_source_bindings_from_expression(callee)
    }

    pub(in crate::backend::direct_wasm) fn resolve_user_constructor_object_binding(
        &self,
        user_function: &UserFunction,
        arguments: &[CallArgument],
        capture_source_bindings: Option<&HashMap<String, Expression>>,
        current_this_binding: ObjectValueBinding,
    ) -> Option<ObjectValueBinding> {
        self.compiler
            .resolve_user_constructor_object_binding_for_function_with_this_binding(
                user_function,
                arguments,
                capture_source_bindings,
                current_this_binding,
            )
    }
}
