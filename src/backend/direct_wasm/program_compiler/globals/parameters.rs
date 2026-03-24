use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn collect_user_function_parameter_bindings(
        &self,
        program: &Program,
    ) -> (
        HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        HashMap<String, HashMap<String, Option<Expression>>>,
        HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
    ) {
        let value_bindings = self.collect_user_function_parameter_value_bindings(program);
        let mut bindings = HashMap::new();
        let mut array_bindings = HashMap::new();
        let mut object_bindings = HashMap::new();
        for function in &program.functions {
            bindings.insert(function.name.clone(), HashMap::new());
            array_bindings.insert(function.name.clone(), HashMap::new());
            object_bindings.insert(function.name.clone(), HashMap::new());
        }
        let mut top_level_aliases = HashMap::new();
        let mut top_level_value_bindings = self.global_value_bindings.clone();
        let mut top_level_object_state = self.global_object_bindings.clone();
        for statement in &program.statements {
            let aliases_before_statement = top_level_aliases.clone();
            let value_bindings_before_statement = top_level_value_bindings.clone();
            let object_state_before_statement = top_level_object_state.clone();
            self.collect_parameter_bindings_from_statement(
                statement,
                &mut top_level_aliases,
                &mut bindings,
                &mut array_bindings,
                &mut object_bindings,
            );
            self.collect_stateful_callback_bindings_from_statement(
                statement,
                &aliases_before_statement,
                &mut bindings,
                &mut array_bindings,
                &mut object_bindings,
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
                &mut bindings,
                &mut array_bindings,
                &mut object_bindings,
            );
        }

        (bindings, value_bindings, array_bindings, object_bindings)
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

    pub(in crate::backend::direct_wasm) fn collect_parameter_value_bindings_from_statements(
        &self,
        statements: &[Statement],
        aliases: &mut HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<Expression>>>,
    ) {
        for statement in statements {
            self.collect_parameter_value_bindings_from_statement(statement, aliases, bindings);
        }
    }

    pub(in crate::backend::direct_wasm) fn collect_parameter_value_bindings_from_statement(
        &self,
        statement: &Statement,
        aliases: &mut HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<Expression>>>,
    ) {
        match statement {
            Statement::Block { body }
            | Statement::Labeled { body, .. }
            | Statement::With { body, .. } => {
                self.collect_parameter_value_bindings_from_statements(body, aliases, bindings);
            }
            Statement::Var { name, value } | Statement::Let { name, value, .. } => {
                self.collect_parameter_value_bindings_from_expression(value, aliases, bindings);
                aliases.insert(
                    name.clone(),
                    self.resolve_function_binding_from_expression_with_aliases(value, aliases),
                );
            }
            Statement::Assign { name, value } => {
                self.collect_parameter_value_bindings_from_expression(value, aliases, bindings);
                aliases.insert(
                    name.clone(),
                    self.resolve_function_binding_from_expression_with_aliases(value, aliases),
                );
            }
            Statement::AssignMember {
                object,
                property,
                value,
            } => {
                self.collect_parameter_value_bindings_from_expression(object, aliases, bindings);
                self.collect_parameter_value_bindings_from_expression(property, aliases, bindings);
                self.collect_parameter_value_bindings_from_expression(value, aliases, bindings);
            }
            Statement::Print { values } => {
                for value in values {
                    self.collect_parameter_value_bindings_from_expression(value, aliases, bindings);
                }
            }
            Statement::Expression(expression)
            | Statement::Throw(expression)
            | Statement::Return(expression)
            | Statement::Yield { value: expression }
            | Statement::YieldDelegate { value: expression } => {
                self.collect_parameter_value_bindings_from_expression(
                    expression, aliases, bindings,
                );
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.collect_parameter_value_bindings_from_expression(condition, aliases, bindings);
                let baseline_aliases = aliases.clone();
                let mut then_aliases = baseline_aliases.clone();
                let mut else_aliases = baseline_aliases.clone();
                self.collect_parameter_value_bindings_from_statements(
                    then_branch,
                    &mut then_aliases,
                    bindings,
                );
                self.collect_parameter_value_bindings_from_statements(
                    else_branch,
                    &mut else_aliases,
                    bindings,
                );
                *aliases = self
                    .merge_aliases_for_branches(&baseline_aliases, &[&then_aliases, &else_aliases]);
            }
            Statement::Try {
                body,
                catch_binding,
                catch_setup,
                catch_body,
            } => {
                let baseline_aliases = aliases.clone();
                let mut body_aliases = baseline_aliases.clone();
                self.collect_parameter_value_bindings_from_statements(
                    body,
                    &mut body_aliases,
                    bindings,
                );

                let mut catch_aliases = baseline_aliases.clone();
                if let Some(binding) = catch_binding {
                    catch_aliases.insert(binding.clone(), None);
                }
                self.collect_parameter_value_bindings_from_statements(
                    catch_setup,
                    &mut catch_aliases,
                    bindings,
                );
                self.collect_parameter_value_bindings_from_statements(
                    catch_body,
                    &mut catch_aliases,
                    bindings,
                );
                *aliases = self.merge_aliases_for_branches(
                    &baseline_aliases,
                    &[&body_aliases, &catch_aliases],
                );
            }
            Statement::Switch {
                discriminant,
                cases,
                ..
            } => {
                self.collect_parameter_value_bindings_from_expression(
                    discriminant,
                    aliases,
                    bindings,
                );
                let baseline_aliases = aliases.clone();
                let mut branch_aliases = Vec::new();
                for case in cases {
                    let mut case_aliases = baseline_aliases.clone();
                    if let Some(test) = &case.test {
                        self.collect_parameter_value_bindings_from_expression(
                            test,
                            &mut case_aliases,
                            bindings,
                        );
                    }
                    self.collect_parameter_value_bindings_from_statements(
                        &case.body,
                        &mut case_aliases,
                        bindings,
                    );
                    branch_aliases.push(case_aliases);
                }
                let branch_refs = branch_aliases.iter().collect::<Vec<_>>();
                *aliases = self.merge_aliases_for_branches(&baseline_aliases, &branch_refs);
            }
            Statement::For {
                init,
                condition,
                update,
                break_hook,
                body,
                ..
            } => {
                self.collect_parameter_value_bindings_from_statements(init, aliases, bindings);
                if let Some(condition) = condition {
                    self.collect_parameter_value_bindings_from_expression(
                        condition, aliases, bindings,
                    );
                }
                if let Some(update) = update {
                    self.collect_parameter_value_bindings_from_expression(
                        update, aliases, bindings,
                    );
                }
                if let Some(break_hook) = break_hook {
                    self.collect_parameter_value_bindings_from_expression(
                        break_hook, aliases, bindings,
                    );
                }
                let baseline_aliases = aliases.clone();
                let mut body_aliases = baseline_aliases.clone();
                self.collect_parameter_value_bindings_from_statements(
                    body,
                    &mut body_aliases,
                    bindings,
                );
                *aliases = self.merge_aliases_for_optional_body(&baseline_aliases, &body_aliases);
            }
            Statement::While {
                condition,
                break_hook,
                body,
                ..
            }
            | Statement::DoWhile {
                condition,
                break_hook,
                body,
                ..
            } => {
                self.collect_parameter_value_bindings_from_expression(condition, aliases, bindings);
                if let Some(break_hook) = break_hook {
                    self.collect_parameter_value_bindings_from_expression(
                        break_hook, aliases, bindings,
                    );
                }
                let baseline_aliases = aliases.clone();
                let mut body_aliases = baseline_aliases.clone();
                self.collect_parameter_value_bindings_from_statements(
                    body,
                    &mut body_aliases,
                    bindings,
                );
                *aliases = self.merge_aliases_for_optional_body(&baseline_aliases, &body_aliases);
            }
            Statement::Break { .. } | Statement::Continue { .. } => {}
        }
    }

    pub(in crate::backend::direct_wasm) fn collect_parameter_value_bindings_from_expression(
        &self,
        expression: &Expression,
        aliases: &mut HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<Expression>>>,
    ) {
        match expression {
            Expression::Call { callee, arguments } => {
                self.collect_parameter_value_bindings_from_expression(callee, aliases, bindings);
                self.register_parameter_value_bindings_for_call(
                    callee, arguments, aliases, bindings,
                );
                for argument in arguments {
                    let argument = match argument {
                        CallArgument::Expression(argument) | CallArgument::Spread(argument) => {
                            argument
                        }
                    };
                    self.collect_parameter_value_bindings_from_expression(
                        argument, aliases, bindings,
                    );
                }
            }
            Expression::Assign { name, value } => {
                self.collect_parameter_value_bindings_from_expression(value, aliases, bindings);
                aliases.insert(
                    name.clone(),
                    self.resolve_function_binding_from_expression_with_aliases(value, aliases),
                );
            }
            Expression::Member { object, property } => {
                self.collect_parameter_value_bindings_from_expression(object, aliases, bindings);
                self.collect_parameter_value_bindings_from_expression(property, aliases, bindings);
            }
            Expression::SuperMember { property } => {
                self.collect_parameter_value_bindings_from_expression(property, aliases, bindings);
            }
            Expression::AssignMember {
                object,
                property,
                value,
            } => {
                self.collect_parameter_value_bindings_from_expression(object, aliases, bindings);
                self.collect_parameter_value_bindings_from_expression(property, aliases, bindings);
                self.collect_parameter_value_bindings_from_expression(value, aliases, bindings);
            }
            Expression::AssignSuperMember { property, value } => {
                self.collect_parameter_value_bindings_from_expression(property, aliases, bindings);
                self.collect_parameter_value_bindings_from_expression(value, aliases, bindings);
            }
            Expression::Unary { expression, .. }
            | Expression::Await(expression)
            | Expression::EnumerateKeys(expression)
            | Expression::GetIterator(expression)
            | Expression::IteratorClose(expression) => {
                self.collect_parameter_value_bindings_from_expression(
                    expression, aliases, bindings,
                );
            }
            Expression::Array(elements) => {
                for element in elements {
                    let expression = match element {
                        ArrayElement::Expression(expression) | ArrayElement::Spread(expression) => {
                            expression
                        }
                    };
                    self.collect_parameter_value_bindings_from_expression(
                        expression, aliases, bindings,
                    );
                }
            }
            Expression::Object(entries) => {
                for entry in entries {
                    match entry {
                        ObjectEntry::Data { key, value } => {
                            self.collect_parameter_value_bindings_from_expression(
                                key, aliases, bindings,
                            );
                            self.collect_parameter_value_bindings_from_expression(
                                value, aliases, bindings,
                            );
                        }
                        ObjectEntry::Getter { key, getter } => {
                            self.collect_parameter_value_bindings_from_expression(
                                key, aliases, bindings,
                            );
                            self.collect_parameter_value_bindings_from_expression(
                                getter, aliases, bindings,
                            );
                        }
                        ObjectEntry::Setter { key, setter } => {
                            self.collect_parameter_value_bindings_from_expression(
                                key, aliases, bindings,
                            );
                            self.collect_parameter_value_bindings_from_expression(
                                setter, aliases, bindings,
                            );
                        }
                        ObjectEntry::Spread(expression) => {
                            self.collect_parameter_value_bindings_from_expression(
                                expression, aliases, bindings,
                            );
                        }
                    }
                }
            }
            Expression::Binary { left, right, .. } => {
                self.collect_parameter_value_bindings_from_expression(left, aliases, bindings);
                self.collect_parameter_value_bindings_from_expression(right, aliases, bindings);
            }
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                self.collect_parameter_value_bindings_from_expression(condition, aliases, bindings);
                self.collect_parameter_value_bindings_from_expression(
                    then_expression,
                    aliases,
                    bindings,
                );
                self.collect_parameter_value_bindings_from_expression(
                    else_expression,
                    aliases,
                    bindings,
                );
            }
            Expression::Sequence(expressions) => {
                for expression in expressions {
                    self.collect_parameter_value_bindings_from_expression(
                        expression, aliases, bindings,
                    );
                }
            }
            Expression::New { callee, arguments } | Expression::SuperCall { callee, arguments } => {
                self.collect_parameter_value_bindings_from_expression(callee, aliases, bindings);
                for argument in arguments {
                    let argument = match argument {
                        CallArgument::Expression(argument) | CallArgument::Spread(argument) => {
                            argument
                        }
                    };
                    self.collect_parameter_value_bindings_from_expression(
                        argument, aliases, bindings,
                    );
                }
            }
            Expression::Update { .. }
            | Expression::Number(_)
            | Expression::BigInt(_)
            | Expression::String(_)
            | Expression::Bool(_)
            | Expression::Null
            | Expression::Undefined
            | Expression::Identifier(_)
            | Expression::This
            | Expression::Sent
            | Expression::NewTarget => {}
        }
    }

    pub(in crate::backend::direct_wasm) fn register_parameter_value_bindings_for_call(
        &self,
        callee: &Expression,
        arguments: &[CallArgument],
        aliases: &HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<Expression>>>,
    ) {
        let (called_function_name, call_arguments) = match callee {
            Expression::Member { object, property } if matches!(property.as_ref(), Expression::String(name) if name == "call") =>
            {
                let Some(LocalFunctionBinding::User(called_function_name)) =
                    self.resolve_function_binding_from_expression_with_aliases(object, aliases)
                else {
                    return;
                };
                (
                    called_function_name,
                    expand_static_call_arguments(arguments, &self.global_array_bindings)
                        .into_iter()
                        .skip(1)
                        .collect::<Vec<_>>(),
                )
            }
            Expression::Member { object, property } if matches!(property.as_ref(), Expression::String(name) if name == "apply") =>
            {
                let Some(LocalFunctionBinding::User(called_function_name)) =
                    self.resolve_function_binding_from_expression_with_aliases(object, aliases)
                else {
                    return;
                };
                let expanded_arguments =
                    expand_static_call_arguments(arguments, &self.global_array_bindings);
                let apply_expression = expanded_arguments
                    .get(1)
                    .cloned()
                    .unwrap_or(Expression::Undefined);
                let Some(call_arguments) =
                    self.expand_apply_parameter_call_arguments_from_expression(&apply_expression)
                else {
                    return;
                };
                (called_function_name, call_arguments)
            }
            _ => {
                let Some(LocalFunctionBinding::User(called_function_name)) =
                    self.resolve_function_binding_from_expression_with_aliases(callee, aliases)
                else {
                    return;
                };
                (
                    called_function_name,
                    expand_static_call_arguments(arguments, &self.global_array_bindings),
                )
            }
        };
        let Some(user_function) = self.user_function_map.get(&called_function_name) else {
            return;
        };
        let Some(parameter_bindings) = bindings.get_mut(&called_function_name) else {
            return;
        };

        for (index, argument) in call_arguments.iter().enumerate() {
            if index >= user_function.params.len() {
                break;
            }
            let param_name = &user_function.params[index];
            let materialized_argument = self
                .infer_global_object_binding(argument)
                .map(|binding| object_binding_to_expression(&binding))
                .unwrap_or_else(|| self.materialize_global_expression(argument));
            match parameter_bindings.get(param_name) {
                Some(None) => {}
                Some(Some(existing)) if *existing == materialized_argument => {}
                Some(Some(_)) => {
                    parameter_bindings.insert(param_name.to_string(), None);
                }
                None => {
                    parameter_bindings.insert(param_name.to_string(), Some(materialized_argument));
                }
            }
        }

        if call_arguments.len() < user_function.params.len() {
            for param_name in user_function.params.iter().skip(call_arguments.len()) {
                parameter_bindings.insert(param_name.to_string(), None);
            }
        }
    }

    pub(in crate::backend::direct_wasm) fn merge_aliases_for_branches(
        &self,
        baseline: &HashMap<String, Option<LocalFunctionBinding>>,
        branches: &[&HashMap<String, Option<LocalFunctionBinding>>],
    ) -> HashMap<String, Option<LocalFunctionBinding>> {
        let mut merged = baseline.clone();
        for (name, baseline_binding) in baseline {
            for branch in branches {
                if branch.get(name) != Some(baseline_binding) {
                    merged.insert(name.clone(), None);
                    break;
                }
            }
        }
        merged
    }

    pub(in crate::backend::direct_wasm) fn merge_aliases_for_optional_body(
        &self,
        before_body: &HashMap<String, Option<LocalFunctionBinding>>,
        after_body: &HashMap<String, Option<LocalFunctionBinding>>,
    ) -> HashMap<String, Option<LocalFunctionBinding>> {
        self.merge_aliases_for_branches(before_body, &[before_body, after_body])
    }

    pub(in crate::backend::direct_wasm) fn collect_parameter_bindings_from_statements(
        &self,
        statements: &[Statement],
        aliases: &mut HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
    ) {
        for statement in statements {
            self.collect_parameter_bindings_from_statement(
                statement,
                aliases,
                bindings,
                array_bindings,
                object_bindings,
            );
        }
    }

    pub(in crate::backend::direct_wasm) fn collect_parameter_bindings_from_statement(
        &self,
        statement: &Statement,
        aliases: &mut HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
    ) {
        match statement {
            Statement::Block { body } | Statement::Labeled { body, .. } => {
                self.collect_parameter_bindings_from_statements(
                    body,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
            }
            Statement::Var { name, value } | Statement::Let { name, value, .. } => {
                self.collect_parameter_bindings_from_expression(
                    value,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
                let function_binding =
                    self.resolve_function_binding_from_expression_with_aliases(value, aliases);
                aliases.insert(name.clone(), function_binding);
            }
            Statement::Assign { name, value } => {
                self.collect_parameter_bindings_from_expression(
                    value,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
                let function_binding =
                    self.resolve_function_binding_from_expression_with_aliases(value, aliases);
                aliases.insert(name.clone(), function_binding);
            }
            Statement::Yield { value } => {
                self.collect_parameter_bindings_from_expression(
                    value,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
            }
            Statement::YieldDelegate { value } => {
                self.collect_parameter_bindings_from_expression(
                    value,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
            }
            Statement::AssignMember {
                object,
                property,
                value,
            } => {
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
            Statement::Print { values } => {
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
            Statement::Expression(expression)
            | Statement::Throw(expression)
            | Statement::Return(expression) => {
                self.collect_parameter_bindings_from_expression(
                    expression,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => {
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
                *aliases = self
                    .merge_aliases_for_branches(&baseline_aliases, &[&then_aliases, &else_aliases]);
            }
            Statement::While {
                condition,
                body,
                break_hook,
                ..
            } => {
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
            Statement::DoWhile {
                condition,
                body,
                break_hook,
                ..
            } => {
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
            Statement::For {
                init,
                condition,
                update,
                body,
                break_hook,
                per_iteration_bindings,
                ..
            } => {
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
                *aliases = self.merge_aliases_for_optional_body(&aliases, &loop_aliases);
            }
            Statement::With { object, body } => {
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
            Statement::Try {
                body,
                catch_setup,
                catch_binding,
                catch_body,
                ..
            } => {
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
                *aliases = self
                    .merge_aliases_for_branches(&baseline_aliases, &[&try_aliases, &catch_aliases]);
            }
            Statement::Switch {
                discriminant,
                cases,
                bindings: case_bindings,
                ..
            } => {
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
                    merged_aliases =
                        self.merge_aliases_for_branches(&merged_aliases, &[&case_aliases]);
                }
                *aliases = merged_aliases;
            }
            Statement::Break { .. } | Statement::Continue { .. } => {}
        }
    }

    pub(in crate::backend::direct_wasm) fn collect_parameter_bindings_from_expression(
        &self,
        expression: &Expression,
        aliases: &mut HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
    ) {
        match expression {
            Expression::Call { callee, arguments } => {
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
                for argument in arguments {
                    match argument {
                        CallArgument::Expression(argument) => {
                            self.collect_parameter_bindings_from_expression(
                                argument,
                                aliases,
                                bindings,
                                array_bindings,
                                object_bindings,
                            );
                        }
                        CallArgument::Spread(argument) => {
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
            Expression::Assign { name, value } => {
                self.collect_parameter_bindings_from_expression(
                    value,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
                let function_binding =
                    self.resolve_function_binding_from_expression_with_aliases(value, aliases);
                aliases.insert(name.clone(), function_binding);
            }
            Expression::Member { object, property } => {
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
            }
            Expression::SuperMember { property } => {
                self.collect_parameter_bindings_from_expression(
                    property,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
            }
            Expression::AssignMember {
                object,
                property,
                value,
            } => {
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
            Expression::AssignSuperMember { property, value } => {
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
            Expression::Unary { expression, .. } => self
                .collect_parameter_bindings_from_expression(
                    expression,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                ),
            Expression::Array(elements) => {
                for element in elements {
                    match element {
                        crate::ir::hir::ArrayElement::Expression(expression) => {
                            self.collect_parameter_bindings_from_expression(
                                expression,
                                aliases,
                                bindings,
                                array_bindings,
                                object_bindings,
                            );
                        }
                        crate::ir::hir::ArrayElement::Spread(expression) => {
                            self.collect_parameter_bindings_from_expression(
                                expression,
                                aliases,
                                bindings,
                                array_bindings,
                                object_bindings,
                            );
                        }
                    }
                }
            }
            Expression::Object(entries) => {
                for entry in entries {
                    match entry {
                        crate::ir::hir::ObjectEntry::Data { key, value } => {
                            self.collect_parameter_bindings_from_expression(
                                key,
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
                        crate::ir::hir::ObjectEntry::Getter { key, getter } => {
                            self.collect_parameter_bindings_from_expression(
                                key,
                                aliases,
                                bindings,
                                array_bindings,
                                object_bindings,
                            );
                            self.collect_parameter_bindings_from_expression(
                                getter,
                                aliases,
                                bindings,
                                array_bindings,
                                object_bindings,
                            );
                        }
                        crate::ir::hir::ObjectEntry::Setter { key, setter } => {
                            self.collect_parameter_bindings_from_expression(
                                key,
                                aliases,
                                bindings,
                                array_bindings,
                                object_bindings,
                            );
                            self.collect_parameter_bindings_from_expression(
                                setter,
                                aliases,
                                bindings,
                                array_bindings,
                                object_bindings,
                            );
                        }
                        crate::ir::hir::ObjectEntry::Spread(expression) => {
                            self.collect_parameter_bindings_from_expression(
                                expression,
                                aliases,
                                bindings,
                                array_bindings,
                                object_bindings,
                            );
                        }
                    }
                }
            }
            Expression::EnumerateKeys(expression)
            | Expression::GetIterator(expression)
            | Expression::IteratorClose(expression)
            | Expression::Await(expression) => {
                self.collect_parameter_bindings_from_expression(
                    expression,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
            }
            Expression::NewTarget => {}
            Expression::Binary { left, right, .. } => {
                self.collect_parameter_bindings_from_expression(
                    left,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
                self.collect_parameter_bindings_from_expression(
                    right,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
            }
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                self.collect_parameter_bindings_from_expression(
                    condition,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
                self.collect_parameter_bindings_from_expression(
                    then_expression,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
                self.collect_parameter_bindings_from_expression(
                    else_expression,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
            }
            Expression::Sequence(expressions) => {
                for expression in expressions {
                    self.collect_parameter_bindings_from_expression(
                        expression,
                        aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                    );
                }
            }
            Expression::New { callee, arguments } => {
                self.collect_parameter_bindings_from_expression(
                    callee,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
                for argument in arguments {
                    match argument {
                        CallArgument::Expression(argument) => {
                            self.collect_parameter_bindings_from_expression(
                                argument,
                                aliases,
                                bindings,
                                array_bindings,
                                object_bindings,
                            );
                        }
                        CallArgument::Spread(argument) => {
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
            Expression::SuperCall { callee, arguments } => {
                self.collect_parameter_bindings_from_expression(
                    callee,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                );
                for argument in arguments {
                    match argument {
                        CallArgument::Expression(argument) => {
                            self.collect_parameter_bindings_from_expression(
                                argument,
                                aliases,
                                bindings,
                                array_bindings,
                                object_bindings,
                            );
                        }
                        CallArgument::Spread(argument) => {
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
            Expression::Update { .. }
            | Expression::Number(_)
            | Expression::BigInt(_)
            | Expression::String(_)
            | Expression::Bool(_)
            | Expression::Null
            | Expression::Undefined
            | Expression::Identifier(_)
            | Expression::This
            | Expression::Sent => {}
        }
    }
}
