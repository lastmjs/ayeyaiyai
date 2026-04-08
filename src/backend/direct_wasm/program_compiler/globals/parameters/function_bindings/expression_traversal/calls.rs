use super::super::super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn handle_call_parameter_expression(
        &self,
        callee: &Expression,
        arguments: &[CallArgument],
        aliases: &mut HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
    ) {
        self.collect_parameter_bindings_from_expression(
            callee,
            aliases,
            bindings,
            array_bindings,
            object_bindings,
        );
        self.register_callback_bindings_for_call(
            callee,
            arguments,
            aliases,
            bindings,
            array_bindings,
            object_bindings,
        );
        self.collect_parameter_bindings_from_call_arguments(
            arguments,
            aliases,
            bindings,
            array_bindings,
            object_bindings,
        );
    }

    pub(in crate::backend::direct_wasm) fn handle_construct_parameter_expression(
        &self,
        callee: &Expression,
        arguments: &[CallArgument],
        aliases: &mut HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
    ) {
        self.collect_parameter_bindings_from_expression(
            callee,
            aliases,
            bindings,
            array_bindings,
            object_bindings,
        );
        self.collect_parameter_bindings_from_call_arguments(
            arguments,
            aliases,
            bindings,
            array_bindings,
            object_bindings,
        );
    }

    fn collect_parameter_bindings_from_call_arguments(
        &self,
        arguments: &[CallArgument],
        aliases: &mut HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
    ) {
        for argument in arguments {
            match argument {
                CallArgument::Expression(argument) | CallArgument::Spread(argument) => {
                    self.collect_parameter_bindings_from_expression(
                        argument,
                        aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                    );
                }
            }
        }
    }
}
