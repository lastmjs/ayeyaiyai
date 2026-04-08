use super::*;

impl DirectWasmCompiler {
    pub(super) fn handle_while_callback_statement(
        &self,
        condition: &Expression,
        body: &[Statement],
        break_hook: Option<&Expression>,
        aliases: &HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
        value_bindings: &HashMap<String, Expression>,
        object_state: &HashMap<String, ObjectValueBinding>,
        overwrite_existing: bool,
    ) {
        self.collect_stateful_callback_loop_parts(
            condition,
            body,
            break_hook,
            aliases,
            bindings,
            array_bindings,
            object_bindings,
            value_bindings,
            object_state,
            overwrite_existing,
        );
    }

    pub(super) fn handle_do_while_callback_statement(
        &self,
        condition: &Expression,
        body: &[Statement],
        break_hook: Option<&Expression>,
        aliases: &HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
        value_bindings: &HashMap<String, Expression>,
        object_state: &HashMap<String, ObjectValueBinding>,
        overwrite_existing: bool,
    ) {
        self.collect_stateful_callback_loop_parts(
            condition,
            body,
            break_hook,
            aliases,
            bindings,
            array_bindings,
            object_bindings,
            value_bindings,
            object_state,
            overwrite_existing,
        );
    }

    pub(super) fn handle_for_callback_statement(
        &self,
        init: &[Statement],
        condition: Option<&Expression>,
        update: Option<&Expression>,
        body: &[Statement],
        break_hook: Option<&Expression>,
        aliases: &HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
        value_bindings: &HashMap<String, Expression>,
        object_state: &HashMap<String, ObjectValueBinding>,
        overwrite_existing: bool,
    ) {
        self.collect_stateful_callback_bindings_from_statements(
            init,
            aliases,
            bindings,
            array_bindings,
            object_bindings,
            value_bindings,
            object_state,
            overwrite_existing,
        );
        if let Some(condition) = condition {
            self.collect_stateful_callback_bindings_from_expression(
                condition,
                aliases,
                bindings,
                array_bindings,
                object_bindings,
                value_bindings,
                object_state,
                overwrite_existing,
            );
        }
        if let Some(update) = update {
            self.collect_stateful_callback_bindings_from_expression(
                update,
                aliases,
                bindings,
                array_bindings,
                object_bindings,
                value_bindings,
                object_state,
                overwrite_existing,
            );
        }
        if let Some(break_hook) = break_hook {
            self.collect_stateful_callback_bindings_from_expression(
                break_hook,
                aliases,
                bindings,
                array_bindings,
                object_bindings,
                value_bindings,
                object_state,
                overwrite_existing,
            );
        }
        self.collect_stateful_callback_bindings_from_statements(
            body,
            aliases,
            bindings,
            array_bindings,
            object_bindings,
            value_bindings,
            object_state,
            overwrite_existing,
        );
    }

    fn collect_stateful_callback_loop_parts(
        &self,
        condition: &Expression,
        body: &[Statement],
        break_hook: Option<&Expression>,
        aliases: &HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
        value_bindings: &HashMap<String, Expression>,
        object_state: &HashMap<String, ObjectValueBinding>,
        overwrite_existing: bool,
    ) {
        self.collect_stateful_callback_bindings_from_expression(
            condition,
            aliases,
            bindings,
            array_bindings,
            object_bindings,
            value_bindings,
            object_state,
            overwrite_existing,
        );
        if let Some(break_hook) = break_hook {
            self.collect_stateful_callback_bindings_from_expression(
                break_hook,
                aliases,
                bindings,
                array_bindings,
                object_bindings,
                value_bindings,
                object_state,
                overwrite_existing,
            );
        }
        self.collect_stateful_callback_bindings_from_statements(
            body,
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
