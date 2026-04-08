use super::super::super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn handle_binding_assignment_parameter_statement(
        &self,
        name: &str,
        value: &Expression,
        aliases: &mut HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
    ) {
        self.collect_parameter_bindings_from_expression(
            value,
            aliases,
            bindings,
            array_bindings,
            object_bindings,
        );
        let function_binding =
            self.resolve_function_binding_from_expression_with_aliases(value, aliases);
        aliases.insert(name.to_string(), function_binding);
    }

    pub(in crate::backend::direct_wasm) fn handle_assign_member_parameter_statement(
        &self,
        object: &Expression,
        property: &Expression,
        value: &Expression,
        aliases: &mut HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
    ) {
        self.collect_parameter_bindings_from_expression(
            object,
            aliases,
            bindings,
            array_bindings,
            object_bindings,
        );
        self.collect_parameter_bindings_from_expression(
            property,
            aliases,
            bindings,
            array_bindings,
            object_bindings,
        );
        self.collect_parameter_bindings_from_expression(
            value,
            aliases,
            bindings,
            array_bindings,
            object_bindings,
        );
    }

    pub(in crate::backend::direct_wasm) fn handle_print_parameter_statement(
        &self,
        values: &[Expression],
        aliases: &mut HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
    ) {
        for value in values {
            self.collect_parameter_bindings_from_expression(
                value,
                aliases,
                bindings,
                array_bindings,
                object_bindings,
            );
        }
    }
}
