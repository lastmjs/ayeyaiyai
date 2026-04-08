use super::super::*;
use super::ProgramStaticEvalContext;

impl StaticBuiltinArrayBindingSource for ProgramStaticEvalContext<'_> {
    fn static_array_binding(&self, expression: &Expression) -> Option<ArrayValueBinding> {
        self.infer_array_binding(expression)
    }

    fn static_object_binding(&self, expression: &Expression) -> Option<ObjectValueBinding> {
        self.infer_object_binding(expression)
    }

    fn has_static_function_binding_source(&self, expression: &Expression) -> bool {
        self.has_function_binding(expression)
    }

    fn has_static_prototype_object_binding_source(&self, name: &str) -> bool {
        self.has_prototype_object_binding(name)
    }
}
