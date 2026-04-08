use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn collect_user_function_parameter_analysis(
        &self,
        program: &Program,
    ) -> UserFunctionParameterAnalysis {
        let value_bindings = self.collect_user_function_parameter_value_bindings(program);
        let mut function_bindings_by_function = HashMap::new();
        let mut array_bindings_by_function = HashMap::new();
        let mut object_bindings_by_function = HashMap::new();
        for function in &program.functions {
            function_bindings_by_function.insert(function.name.clone(), HashMap::new());
            array_bindings_by_function.insert(function.name.clone(), HashMap::new());
            object_bindings_by_function.insert(function.name.clone(), HashMap::new());
        }
        let mut top_level_aliases = HashMap::new();
        let (mut top_level_value_bindings, mut top_level_object_state) =
            self.snapshot_top_level_static_state();
        for statement in &program.statements {
            let aliases_before_statement = top_level_aliases.clone();
            let value_bindings_before_statement = top_level_value_bindings.clone();
            let object_state_before_statement = top_level_object_state.clone();
            self.collect_parameter_bindings_from_statement(
                statement,
                &mut top_level_aliases,
                &mut function_bindings_by_function,
                &mut array_bindings_by_function,
                &mut object_bindings_by_function,
            );
            self.collect_stateful_callback_bindings_from_statement(
                statement,
                &aliases_before_statement,
                &mut function_bindings_by_function,
                &mut array_bindings_by_function,
                &mut object_bindings_by_function,
                &value_bindings_before_statement,
                &object_state_before_statement,
                true,
            );
            self.update_parameter_binding_state_from_statement(
                statement,
                &mut top_level_value_bindings,
                &mut top_level_object_state,
            );
        }
        for function in &program.functions {
            let mut aliases = top_level_aliases.clone();
            for parameter in &function.params {
                aliases.entry(parameter.name.clone()).or_insert(None);
            }
            self.collect_parameter_bindings_from_statements(
                &function.body,
                &mut aliases,
                &mut function_bindings_by_function,
                &mut array_bindings_by_function,
                &mut object_bindings_by_function,
            );
        }

        UserFunctionParameterAnalysis {
            function_bindings_by_function,
            value_bindings_by_function: value_bindings,
            array_bindings_by_function,
            object_bindings_by_function,
        }
    }

    #[cfg(test)]
    pub(in crate::backend::direct_wasm) fn collect_user_function_parameter_bindings(
        &self,
        program: &Program,
    ) -> (
        HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        HashMap<String, HashMap<String, Option<Expression>>>,
        HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
    ) {
        let analysis = self.collect_user_function_parameter_analysis(program);
        (
            analysis.function_bindings_by_function,
            analysis.value_bindings_by_function,
            analysis.array_bindings_by_function,
            analysis.object_bindings_by_function,
        )
    }

    pub(in crate::backend::direct_wasm) fn collect_user_function_parameter_value_bindings(
        &self,
        program: &Program,
    ) -> HashMap<String, HashMap<String, Option<Expression>>> {
        let mut bindings = HashMap::new();
        for function in &program.functions {
            bindings.insert(function.name.clone(), HashMap::new());
        }

        let mut top_level_aliases = HashMap::new();
        for statement in &program.statements {
            self.collect_parameter_value_bindings_from_statement(
                statement,
                &mut top_level_aliases,
                &mut bindings,
            );
        }

        for function in &program.functions {
            let mut aliases = top_level_aliases.clone();
            for parameter in &function.params {
                aliases.entry(parameter.name.clone()).or_insert(None);
            }
            self.collect_parameter_value_bindings_from_statements(
                &function.body,
                &mut aliases,
                &mut bindings,
            );
        }

        bindings
    }
}
