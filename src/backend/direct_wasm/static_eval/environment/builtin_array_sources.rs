use super::*;

pub(in crate::backend::direct_wasm) trait StaticBuiltinArrayBindingResolver {
    fn resolve_static_array_binding(&self, expression: &Expression) -> Option<ArrayValueBinding>;

    fn resolve_static_object_binding(&self, expression: &Expression) -> Option<ObjectValueBinding>;

    fn has_static_function_binding(&self, expression: &Expression) -> bool;

    fn has_static_prototype_object_binding(&self, name: &str) -> bool;

    fn has_static_function_property_shape(&self, expression: &Expression) -> bool {
        self.has_static_function_binding(expression)
            || matches!(
                expression,
                Expression::Identifier(name) if self.has_static_prototype_object_binding(name)
            )
    }

    fn static_enumerated_keys_binding(&self, expression: &Expression) -> Option<ArrayValueBinding> {
        infer_enumerated_keys_binding_from_expression(
            expression,
            |expression| self.resolve_static_array_binding(expression),
            |expression| self.resolve_static_object_binding(expression),
        )
    }

    fn static_own_property_names_binding(
        &self,
        expression: &Expression,
    ) -> Option<ArrayValueBinding> {
        infer_own_property_names_binding_from_expression(
            expression,
            |expression| self.resolve_static_array_binding(expression),
            |expression| self.resolve_static_object_binding(expression),
            |expression| self.has_static_function_property_shape(expression),
        )
    }

    fn static_own_property_symbols_binding(
        &self,
        expression: &Expression,
    ) -> Option<ArrayValueBinding> {
        infer_own_property_symbols_binding_from_expression(expression, |expression| {
            self.resolve_static_object_binding(expression)
        })
    }

    fn static_builtin_object_array_call_binding(
        &self,
        callee: &Expression,
        arguments: &[CallArgument],
    ) -> Option<ArrayValueBinding> {
        infer_builtin_object_array_call_binding(
            callee,
            arguments,
            |target| self.static_enumerated_keys_binding(target),
            |target| self.static_own_property_names_binding(target),
            |target| self.static_own_property_symbols_binding(target),
        )
    }
}

pub(in crate::backend::direct_wasm) trait StaticBuiltinArrayBindingSource {
    fn static_array_binding(&self, expression: &Expression) -> Option<ArrayValueBinding>;

    fn static_object_binding(&self, expression: &Expression) -> Option<ObjectValueBinding>;

    fn has_static_function_binding_source(&self, expression: &Expression) -> bool;

    fn has_static_prototype_object_binding_source(&self, name: &str) -> bool;
}

impl<T> StaticBuiltinArrayBindingResolver for T
where
    T: StaticBuiltinArrayBindingSource + ?Sized,
{
    fn resolve_static_array_binding(&self, expression: &Expression) -> Option<ArrayValueBinding> {
        self.static_array_binding(expression)
    }

    fn resolve_static_object_binding(&self, expression: &Expression) -> Option<ObjectValueBinding> {
        self.static_object_binding(expression)
    }

    fn has_static_function_binding(&self, expression: &Expression) -> bool {
        self.has_static_function_binding_source(expression)
    }

    fn has_static_prototype_object_binding(&self, name: &str) -> bool {
        self.has_static_prototype_object_binding_source(name)
    }
}
