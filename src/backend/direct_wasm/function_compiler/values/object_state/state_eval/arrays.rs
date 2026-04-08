use super::super::*;
use super::FunctionStaticEvalContext;

impl StaticBuiltinArrayBindingSource for FunctionStaticEvalContext<'_, '_> {
    fn static_array_binding(&self, expression: &Expression) -> Option<ArrayValueBinding> {
        self.resolve_array_binding(expression)
    }

    fn static_object_binding(&self, expression: &Expression) -> Option<ObjectValueBinding> {
        self.resolve_object_binding(expression)
    }

    fn has_static_function_binding_source(&self, expression: &Expression) -> bool {
        self.resolve_function_binding(expression).is_some()
    }

    fn has_static_prototype_object_binding_source(&self, name: &str) -> bool {
        self.has_local_prototype_object_binding(name)
            || self.has_global_prototype_object_binding(name)
    }
}
