use super::super::super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn handle_while_parameter_statement(
        &self,
        condition: &Expression,
        body: &[Statement],
        break_hook: Option<&Expression>,
        aliases: &mut HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
    ) {
        let baseline_aliases = aliases.clone();
        let mut loop_aliases = baseline_aliases.clone();
        self.collect_parameter_bindings_from_expression(
            condition,
            aliases,
            bindings,
            array_bindings,
            object_bindings,
        );
        self.collect_parameter_bindings_from_expression(
            condition,
            &mut loop_aliases,
            bindings,
            array_bindings,
            object_bindings,
        );
        if let Some(break_hook) = break_hook {
            self.collect_parameter_bindings_from_expression(
                break_hook,
                &mut loop_aliases,
                bindings,
                array_bindings,
                object_bindings,
            );
        }
        self.collect_parameter_bindings_from_statements(
            body,
            &mut loop_aliases,
            bindings,
            array_bindings,
            object_bindings,
        );
        *aliases = self.merge_aliases_for_optional_body(&baseline_aliases, &loop_aliases);
    }

    pub(in crate::backend::direct_wasm) fn handle_do_while_parameter_statement(
        &self,
        condition: &Expression,
        body: &[Statement],
        break_hook: Option<&Expression>,
        aliases: &mut HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
    ) {
        let baseline_aliases = aliases.clone();
        let mut loop_aliases = baseline_aliases.clone();
        self.collect_parameter_bindings_from_statements(
            body,
            &mut loop_aliases,
            bindings,
            array_bindings,
            object_bindings,
        );
        self.collect_parameter_bindings_from_expression(
            condition,
            &mut loop_aliases,
            bindings,
            array_bindings,
            object_bindings,
        );
        self.collect_parameter_bindings_from_expression(
            condition,
            aliases,
            bindings,
            array_bindings,
            object_bindings,
        );
        if let Some(break_hook) = break_hook {
            self.collect_parameter_bindings_from_expression(
                break_hook,
                &mut loop_aliases,
                bindings,
                array_bindings,
                object_bindings,
            );
        }
        *aliases = self.merge_aliases_for_optional_body(&baseline_aliases, &loop_aliases);
    }

    pub(in crate::backend::direct_wasm) fn handle_for_parameter_statement(
        &self,
        init: &[Statement],
        condition: Option<&Expression>,
        update: Option<&Expression>,
        body: &[Statement],
        break_hook: Option<&Expression>,
        per_iteration_bindings: &[String],
        aliases: &mut HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
    ) {
        let baseline_aliases = aliases.clone();
        let mut loop_aliases = baseline_aliases.clone();
        for binding in per_iteration_bindings {
            loop_aliases.insert(binding.clone(), None);
        }
        self.collect_parameter_bindings_from_statements(
            init,
            &mut loop_aliases,
            bindings,
            array_bindings,
            object_bindings,
        );
        self.collect_parameter_bindings_from_statements(
            init,
            aliases,
            bindings,
            array_bindings,
            object_bindings,
        );
        if let Some(condition) = condition {
            self.collect_parameter_bindings_from_expression(
                condition,
                aliases,
                bindings,
                array_bindings,
                object_bindings,
            );
            self.collect_parameter_bindings_from_expression(
                condition,
                &mut loop_aliases,
                bindings,
                array_bindings,
                object_bindings,
            );
        }
        self.collect_parameter_bindings_from_statements(
            body,
            &mut loop_aliases,
            bindings,
            array_bindings,
            object_bindings,
        );
        if let Some(update) = update {
            self.collect_parameter_bindings_from_expression(
                update,
                &mut loop_aliases,
                bindings,
                array_bindings,
                object_bindings,
            );
        }
        if let Some(break_hook) = break_hook {
            self.collect_parameter_bindings_from_expression(
                break_hook,
                &mut loop_aliases,
                bindings,
                array_bindings,
                object_bindings,
            );
        }
        *aliases = self.merge_aliases_for_optional_body(aliases, &loop_aliases);
    }
}
