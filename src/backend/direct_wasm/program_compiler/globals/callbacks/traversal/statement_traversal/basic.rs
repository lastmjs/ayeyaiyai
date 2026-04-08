use super::*;

impl DirectWasmCompiler {
    pub(super) fn handle_assign_member_callback_statement(
        &self,
        object: &Expression,
        property: &Expression,
        value: &Expression,
        aliases: &HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
        value_bindings: &HashMap<String, Expression>,
        object_state: &HashMap<String, ObjectValueBinding>,
        overwrite_existing: bool,
    ) {
        self.collect_stateful_callback_bindings_from_expression(
            object,
            aliases,
            bindings,
            array_bindings,
            object_bindings,
            value_bindings,
            object_state,
            overwrite_existing,
        );
        self.collect_stateful_callback_bindings_from_expression(
            property,
            aliases,
            bindings,
            array_bindings,
            object_bindings,
            value_bindings,
            object_state,
            overwrite_existing,
        );
        self.collect_stateful_callback_bindings_from_expression(
            value,
            aliases,
            bindings,
            array_bindings,
            object_bindings,
            value_bindings,
            object_state,
            overwrite_existing,
        );
    }

    pub(super) fn handle_print_callback_statement(
        &self,
        values: &[Expression],
        aliases: &HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
        value_bindings: &HashMap<String, Expression>,
        object_state: &HashMap<String, ObjectValueBinding>,
        overwrite_existing: bool,
    ) {
        for value in values {
            self.collect_stateful_callback_bindings_from_expression(
                value,
                aliases,
                bindings,
                array_bindings,
                object_bindings,
                value_bindings,
                object_state,
                overwrite_existing,
            );
        }
    }
}
