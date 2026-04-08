use super::super::super::*;
use crate::ir::hir::SwitchCase;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn handle_if_parameter_statement(
        &self,
        condition: &Expression,
        then_branch: &[Statement],
        else_branch: &[Statement],
        aliases: &mut HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
    ) {
        self.collect_parameter_bindings_from_expression(
            condition,
            aliases,
            bindings,
            array_bindings,
            object_bindings,
        );
        let baseline_aliases = aliases.clone();
        let mut then_aliases = baseline_aliases.clone();
        let mut else_aliases = baseline_aliases.clone();
        self.collect_parameter_bindings_from_statements(
            then_branch,
            &mut then_aliases,
            bindings,
            array_bindings,
            object_bindings,
        );
        self.collect_parameter_bindings_from_statements(
            else_branch,
            &mut else_aliases,
            bindings,
            array_bindings,
            object_bindings,
        );
        *aliases =
            self.merge_aliases_for_branches(&baseline_aliases, &[&then_aliases, &else_aliases]);
    }

    pub(in crate::backend::direct_wasm) fn handle_with_parameter_statement(
        &self,
        object: &Expression,
        body: &[Statement],
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
        let baseline_aliases = aliases.clone();
        let mut with_aliases = baseline_aliases.clone();
        self.collect_parameter_bindings_from_statements(
            body,
            &mut with_aliases,
            bindings,
            array_bindings,
            object_bindings,
        );
        *aliases = self.merge_aliases_for_optional_body(&baseline_aliases, &with_aliases);
    }

    pub(in crate::backend::direct_wasm) fn handle_try_parameter_statement(
        &self,
        body: &[Statement],
        catch_setup: &[Statement],
        catch_binding: Option<&String>,
        catch_body: &[Statement],
        aliases: &mut HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
    ) {
        let baseline_aliases = aliases.clone();
        let mut try_aliases = baseline_aliases.clone();
        let mut catch_aliases = baseline_aliases.clone();
        if let Some(catch_binding) = catch_binding {
            catch_aliases.insert(catch_binding.clone(), None);
        }
        self.collect_parameter_bindings_from_statements(
            body,
            &mut try_aliases,
            bindings,
            array_bindings,
            object_bindings,
        );
        self.collect_parameter_bindings_from_statements(
            catch_setup,
            &mut catch_aliases,
            bindings,
            array_bindings,
            object_bindings,
        );
        self.collect_parameter_bindings_from_statements(
            catch_body,
            &mut catch_aliases,
            bindings,
            array_bindings,
            object_bindings,
        );
        *aliases =
            self.merge_aliases_for_branches(&baseline_aliases, &[&try_aliases, &catch_aliases]);
    }

    pub(in crate::backend::direct_wasm) fn handle_switch_parameter_statement(
        &self,
        discriminant: &Expression,
        cases: &[SwitchCase],
        case_bindings: &[String],
        aliases: &mut HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
    ) {
        self.collect_parameter_bindings_from_expression(
            discriminant,
            aliases,
            bindings,
            array_bindings,
            object_bindings,
        );
        let baseline_aliases = aliases.clone();
        let mut merged_aliases = baseline_aliases.clone();
        for binding in case_bindings {
            merged_aliases.insert(binding.clone(), None);
        }
        for case in cases {
            let mut case_aliases = merged_aliases.clone();
            if let Some(test) = &case.test {
                self.collect_parameter_bindings_from_expression(
                    test,
                    &mut case_aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
            }
            self.collect_parameter_bindings_from_statements(
                &case.body,
                &mut case_aliases,
                bindings,
                array_bindings,
                object_bindings,
            );
            merged_aliases = self.merge_aliases_for_branches(&merged_aliases, &[&case_aliases]);
        }
        *aliases = merged_aliases;
    }
}
