use super::*;

#[derive(Clone, Copy)]
pub(in crate::backend::direct_wasm) struct FunctionCompilerBehavior {
    pub(in crate::backend::direct_wasm) allow_return: bool,
    pub(in crate::backend::direct_wasm) mapped_arguments: bool,
    pub(in crate::backend::direct_wasm) strict_mode: bool,
}

pub(in crate::backend::direct_wasm) struct FunctionParameterBindingView<'a> {
    pub(in crate::backend::direct_wasm) function_bindings:
        &'a HashMap<String, Option<LocalFunctionBinding>>,
    pub(in crate::backend::direct_wasm) value_bindings: &'a HashMap<String, Option<Expression>>,
    pub(in crate::backend::direct_wasm) array_bindings:
        &'a HashMap<String, Option<ArrayValueBinding>>,
    pub(in crate::backend::direct_wasm) object_bindings:
        &'a HashMap<String, Option<ObjectValueBinding>>,
}

impl<'a> FunctionParameterBindingView<'a> {
    pub(in crate::backend::direct_wasm) fn new(
        function_bindings: &'a HashMap<String, Option<LocalFunctionBinding>>,
        value_bindings: &'a HashMap<String, Option<Expression>>,
        array_bindings: &'a HashMap<String, Option<ArrayValueBinding>>,
        object_bindings: &'a HashMap<String, Option<ObjectValueBinding>>,
    ) -> Self {
        Self {
            function_bindings,
            value_bindings,
            array_bindings,
            object_bindings,
        }
    }
}

pub(in crate::backend::direct_wasm) struct FunctionCompilationRequest<'a> {
    pub(in crate::backend::direct_wasm) user_function: Option<&'a UserFunction>,
    pub(in crate::backend::direct_wasm) declaration: Option<&'a FunctionDeclaration>,
    pub(in crate::backend::direct_wasm) behavior: FunctionCompilerBehavior,
    pub(in crate::backend::direct_wasm) global_binding_environment: &'a GlobalBindingEnvironment,
    pub(in crate::backend::direct_wasm) parameter_bindings: FunctionParameterBindingView<'a>,
}
