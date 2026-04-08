use super::*;
use crate::ir::hir::SwitchCase;

impl DirectWasmCompiler {
    pub(super) fn handle_if_callback_statement(
        &self,
        condition: &Expression,
        then_branch: &[Statement],
        else_branch: &[Statement],
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
        self.collect_stateful_callback_bindings_from_statements(
            then_branch,
            aliases,
            bindings,
            array_bindings,
            object_bindings,
            value_bindings,
            object_state,
            overwrite_existing,
        );
        self.collect_stateful_callback_bindings_from_statements(
            else_branch,
            aliases,
            bindings,
            array_bindings,
            object_bindings,
            value_bindings,
            object_state,
            overwrite_existing,
        );
    }

    pub(super) fn handle_with_callback_statement(
        &self,
        object: &Expression,
        body: &[Statement],
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

    pub(super) fn handle_try_callback_statement(
        &self,
        body: &[Statement],
        catch_setup: &[Statement],
        catch_body: &[Statement],
        aliases: &HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
        value_bindings: &HashMap<String, Expression>,
        object_state: &HashMap<String, ObjectValueBinding>,
        overwrite_existing: bool,
    ) {
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
        self.collect_stateful_callback_bindings_from_statements(
            catch_setup,
            aliases,
            bindings,
            array_bindings,
            object_bindings,
            value_bindings,
            object_state,
            overwrite_existing,
        );
        self.collect_stateful_callback_bindings_from_statements(
            catch_body,
            aliases,
            bindings,
            array_bindings,
            object_bindings,
            value_bindings,
            object_state,
            overwrite_existing,
        );
    }

    pub(super) fn handle_switch_callback_statement(
        &self,
        discriminant: &Expression,
        cases: &[SwitchCase],
        aliases: &HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
        value_bindings: &HashMap<String, Expression>,
        object_state: &HashMap<String, ObjectValueBinding>,
        overwrite_existing: bool,
    ) {
        self.collect_stateful_callback_bindings_from_expression(
            discriminant,
            aliases,
            bindings,
            array_bindings,
            object_bindings,
            value_bindings,
            object_state,
            overwrite_existing,
        );
        for case in cases {
            if let Some(test) = &case.test {
                self.collect_stateful_callback_bindings_from_expression(
                    test,
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
                &case.body,
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
