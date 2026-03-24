use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn collect_stateful_callback_bindings_from_statement(
        &self,
        statement: &Statement,
        aliases: &HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
        value_bindings: &HashMap<String, Expression>,
        object_state: &HashMap<String, ObjectValueBinding>,
        overwrite_existing: bool,
    ) {
        match statement {
            Statement::Block { body } | Statement::Labeled { body, .. } => {
                for statement in body {
                    self.collect_stateful_callback_bindings_from_statement(
                        statement,
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
            Statement::Var { value, .. }
            | Statement::Let { value, .. }
            | Statement::Assign { value, .. }
            | Statement::Yield { value }
            | Statement::YieldDelegate { value }
            | Statement::Expression(value)
            | Statement::Throw(value)
            | Statement::Return(value) => self.collect_stateful_callback_bindings_from_expression(
                value,
                aliases,
                bindings,
                array_bindings,
                object_bindings,
                value_bindings,
                object_state,
                overwrite_existing,
            ),
            Statement::AssignMember {
                object,
                property,
                value,
            } => {
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
            Statement::Print { values } => {
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
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => {
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
                for statement in then_branch {
                    self.collect_stateful_callback_bindings_from_statement(
                        statement,
                        aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                        value_bindings,
                        object_state,
                        overwrite_existing,
                    );
                }
                for statement in else_branch {
                    self.collect_stateful_callback_bindings_from_statement(
                        statement,
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
            Statement::While {
                condition,
                body,
                break_hook,
                ..
            }
            | Statement::DoWhile {
                condition,
                body,
                break_hook,
                ..
            } => {
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
                for statement in body {
                    self.collect_stateful_callback_bindings_from_statement(
                        statement,
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
            Statement::For {
                init,
                condition,
                update,
                body,
                break_hook,
                ..
            } => {
                for statement in init {
                    self.collect_stateful_callback_bindings_from_statement(
                        statement,
                        aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                        value_bindings,
                        object_state,
                        overwrite_existing,
                    );
                }
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
                for statement in body {
                    self.collect_stateful_callback_bindings_from_statement(
                        statement,
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
            Statement::With { object, body } => {
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
                for statement in body {
                    self.collect_stateful_callback_bindings_from_statement(
                        statement,
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
            Statement::Try {
                body,
                catch_setup,
                catch_body,
                ..
            } => {
                for statement in body {
                    self.collect_stateful_callback_bindings_from_statement(
                        statement,
                        aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                        value_bindings,
                        object_state,
                        overwrite_existing,
                    );
                }
                for statement in catch_setup {
                    self.collect_stateful_callback_bindings_from_statement(
                        statement,
                        aliases,
                        bindings,
                        array_bindings,
                        object_bindings,
                        value_bindings,
                        object_state,
                        overwrite_existing,
                    );
                }
                for statement in catch_body {
                    self.collect_stateful_callback_bindings_from_statement(
                        statement,
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
            Statement::Switch {
                discriminant,
                cases,
                ..
            } => {
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
                    for statement in &case.body {
                        self.collect_stateful_callback_bindings_from_statement(
                            statement,
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
            Statement::Break { .. } | Statement::Continue { .. } => {}
        }
    }

    pub(in crate::backend::direct_wasm) fn collect_stateful_callback_bindings_from_expression(
        &self,
        expression: &Expression,
        aliases: &HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
        value_bindings: &HashMap<String, Expression>,
        object_state: &HashMap<String, ObjectValueBinding>,
        overwrite_existing: bool,
    ) {
        match expression {
            Expression::Call { callee, arguments } => {
                self.collect_stateful_callback_bindings_from_expression(
                    callee,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                    value_bindings,
                    object_state,
                    overwrite_existing,
                );
                self.register_callback_bindings_for_call_with_state(
                    callee,
                    arguments,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                    value_bindings,
                    object_state,
                    overwrite_existing,
                );
                for argument in arguments {
                    let argument = match argument {
                        CallArgument::Expression(argument) | CallArgument::Spread(argument) => {
                            argument
                        }
                    };
                    self.collect_stateful_callback_bindings_from_expression(
                        argument,
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
            Expression::Array(elements) => {
                for element in elements {
                    let element = match element {
                        crate::ir::hir::ArrayElement::Expression(element)
                        | crate::ir::hir::ArrayElement::Spread(element) => element,
                    };
                    self.collect_stateful_callback_bindings_from_expression(
                        element,
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
            Expression::Object(entries) => {
                for entry in entries {
                    match entry {
                        crate::ir::hir::ObjectEntry::Data { key, value } => {
                            self.collect_stateful_callback_bindings_from_expression(
                                key,
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
                        crate::ir::hir::ObjectEntry::Getter { key, getter } => {
                            self.collect_stateful_callback_bindings_from_expression(
                                key,
                                aliases,
                                bindings,
                                array_bindings,
                                object_bindings,
                                value_bindings,
                                object_state,
                                overwrite_existing,
                            );
                            self.collect_stateful_callback_bindings_from_expression(
                                getter,
                                aliases,
                                bindings,
                                array_bindings,
                                object_bindings,
                                value_bindings,
                                object_state,
                                overwrite_existing,
                            );
                        }
                        crate::ir::hir::ObjectEntry::Setter { key, setter } => {
                            self.collect_stateful_callback_bindings_from_expression(
                                key,
                                aliases,
                                bindings,
                                array_bindings,
                                object_bindings,
                                value_bindings,
                                object_state,
                                overwrite_existing,
                            );
                            self.collect_stateful_callback_bindings_from_expression(
                                setter,
                                aliases,
                                bindings,
                                array_bindings,
                                object_bindings,
                                value_bindings,
                                object_state,
                                overwrite_existing,
                            );
                        }
                        crate::ir::hir::ObjectEntry::Spread(value) => {
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
            }
            Expression::Member { object, property }
            | Expression::AssignMember {
                object, property, ..
            } => {
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
            }
            Expression::SuperMember { property } => {
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
            }
            Expression::Assign { value, .. }
            | Expression::Await(value)
            | Expression::EnumerateKeys(value)
            | Expression::GetIterator(value)
            | Expression::IteratorClose(value)
            | Expression::Unary {
                expression: value, ..
            } => self.collect_stateful_callback_bindings_from_expression(
                value,
                aliases,
                bindings,
                array_bindings,
                object_bindings,
                value_bindings,
                object_state,
                overwrite_existing,
            ),
            Expression::AssignSuperMember { property, value } => {
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
            Expression::Binary { left, right, .. } => {
                self.collect_stateful_callback_bindings_from_expression(
                    left,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                    value_bindings,
                    object_state,
                    overwrite_existing,
                );
                self.collect_stateful_callback_bindings_from_expression(
                    right,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                    value_bindings,
                    object_state,
                    overwrite_existing,
                );
            }
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
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
                self.collect_stateful_callback_bindings_from_expression(
                    then_expression,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                    value_bindings,
                    object_state,
                    overwrite_existing,
                );
                self.collect_stateful_callback_bindings_from_expression(
                    else_expression,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                    value_bindings,
                    object_state,
                    overwrite_existing,
                );
            }
            Expression::Sequence(expressions) => {
                for expression in expressions {
                    self.collect_stateful_callback_bindings_from_expression(
                        expression,
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
            Expression::New { callee, arguments } | Expression::SuperCall { callee, arguments } => {
                self.collect_stateful_callback_bindings_from_expression(
                    callee,
                    aliases,
                    bindings,
                    array_bindings,
                    object_bindings,
                    value_bindings,
                    object_state,
                    overwrite_existing,
                );
                for argument in arguments {
                    let argument = match argument {
                        CallArgument::Expression(argument) | CallArgument::Spread(argument) => {
                            argument
                        }
                    };
                    self.collect_stateful_callback_bindings_from_expression(
                        argument,
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
            Expression::Update { .. }
            | Expression::Number(_)
            | Expression::BigInt(_)
            | Expression::String(_)
            | Expression::Bool(_)
            | Expression::Null
            | Expression::Undefined
            | Expression::Identifier(_)
            | Expression::This
            | Expression::NewTarget
            | Expression::Sent => {}
        }
    }

    pub(in crate::backend::direct_wasm) fn update_parameter_binding_state_from_statement(
        &self,
        statement: &Statement,
        value_bindings: &mut HashMap<String, Expression>,
        object_bindings: &mut HashMap<String, ObjectValueBinding>,
    ) {
        match statement {
            Statement::Block { body } | Statement::Labeled { body, .. } => {
                for statement in body {
                    self.update_parameter_binding_state_from_statement(
                        statement,
                        value_bindings,
                        object_bindings,
                    );
                }
            }
            Statement::Var { name, value } | Statement::Let { name, value, .. } => {
                let materialized_value = self
                    .materialize_global_expression_with_state(
                        value,
                        &HashMap::new(),
                        value_bindings,
                        object_bindings,
                    )
                    .unwrap_or_else(|| self.materialize_global_expression(value));
                value_bindings.insert(name.clone(), materialized_value.clone());
                if let Some(binding) = self.infer_global_object_binding_with_state(
                    &materialized_value,
                    value_bindings,
                    object_bindings,
                ) {
                    object_bindings.insert(name.clone(), binding);
                } else {
                    object_bindings.remove(name);
                }
            }
            Statement::Assign { name, value } => {
                let materialized_value = self
                    .materialize_global_expression_with_state(
                        value,
                        &HashMap::new(),
                        value_bindings,
                        object_bindings,
                    )
                    .unwrap_or_else(|| self.materialize_global_expression(value));
                value_bindings.insert(name.clone(), materialized_value.clone());
                if let Some(binding) = self.infer_global_object_binding_with_state(
                    &materialized_value,
                    value_bindings,
                    object_bindings,
                ) {
                    object_bindings.insert(name.clone(), binding);
                } else {
                    object_bindings.remove(name);
                }
            }
            Statement::AssignMember {
                object,
                property,
                value,
            } => {
                let materialized_property = self
                    .materialize_global_expression_with_state(
                        property,
                        &HashMap::new(),
                        value_bindings,
                        object_bindings,
                    )
                    .unwrap_or_else(|| self.materialize_global_expression(property));
                let materialized_value = self
                    .materialize_global_expression_with_state(
                        value,
                        &HashMap::new(),
                        value_bindings,
                        object_bindings,
                    )
                    .unwrap_or_else(|| self.materialize_global_expression(value));
                let _ = self.assign_global_member_expression_with_state(
                    object,
                    materialized_property,
                    materialized_value,
                    &mut HashMap::new(),
                    value_bindings,
                    object_bindings,
                );
            }
            Statement::Expression(expression) => self
                .update_parameter_binding_state_from_expression(
                    expression,
                    value_bindings,
                    object_bindings,
                ),
            _ => {}
        }
    }

    pub(in crate::backend::direct_wasm) fn update_parameter_binding_state_from_expression(
        &self,
        expression: &Expression,
        value_bindings: &mut HashMap<String, Expression>,
        object_bindings: &mut HashMap<String, ObjectValueBinding>,
    ) {
        match expression {
            Expression::Assign { name, value } => {
                let materialized_value = self
                    .materialize_global_expression_with_state(
                        value,
                        &HashMap::new(),
                        value_bindings,
                        object_bindings,
                    )
                    .unwrap_or_else(|| self.materialize_global_expression(value));
                value_bindings.insert(name.clone(), materialized_value.clone());
                if let Some(binding) = self.infer_global_object_binding_with_state(
                    &materialized_value,
                    value_bindings,
                    object_bindings,
                ) {
                    object_bindings.insert(name.clone(), binding);
                } else {
                    object_bindings.remove(name);
                }
                return;
            }
            Expression::AssignMember {
                object,
                property,
                value,
            } => {
                let materialized_property = self
                    .materialize_global_expression_with_state(
                        property,
                        &HashMap::new(),
                        value_bindings,
                        object_bindings,
                    )
                    .unwrap_or_else(|| self.materialize_global_expression(property));
                let materialized_value = self
                    .materialize_global_expression_with_state(
                        value,
                        &HashMap::new(),
                        value_bindings,
                        object_bindings,
                    )
                    .unwrap_or_else(|| self.materialize_global_expression(value));
                let _ = self.assign_global_member_expression_with_state(
                    object,
                    materialized_property,
                    materialized_value,
                    &mut HashMap::new(),
                    value_bindings,
                    object_bindings,
                );
                return;
            }
            Expression::Sequence(expressions) => {
                for expression in expressions {
                    self.update_parameter_binding_state_from_expression(
                        expression,
                        value_bindings,
                        object_bindings,
                    );
                }
                return;
            }
            _ => {}
        }

        let Expression::Call { callee, arguments } = expression else {
            return;
        };
        let Expression::Member { object, property } = callee.as_ref() else {
            return;
        };
        if !matches!(object.as_ref(), Expression::Identifier(name) if name == "Object")
            || !matches!(property.as_ref(), Expression::String(name) if name == "defineProperty")
        {
            return;
        }
        let [
            CallArgument::Expression(target),
            CallArgument::Expression(property),
            CallArgument::Expression(descriptor_expression),
            ..,
        ] = arguments.as_slice()
        else {
            return;
        };
        let Some(descriptor) = resolve_property_descriptor_definition(descriptor_expression) else {
            return;
        };
        let Expression::Identifier(name) = target else {
            return;
        };

        let property = self
            .materialize_global_expression_with_state(
                property,
                &HashMap::new(),
                value_bindings,
                object_bindings,
            )
            .unwrap_or_else(|| self.materialize_global_expression(property));
        let property_name = static_property_name_from_expression(&property);
        let existing_value = object_bindings
            .get(name)
            .and_then(|object_binding| object_binding_lookup_value(object_binding, &property))
            .cloned();
        let current_enumerable = property_name.as_ref().is_some_and(|property_name| {
            object_bindings
                .get(name)
                .map(|object_binding| {
                    !object_binding
                        .non_enumerable_string_properties
                        .iter()
                        .any(|hidden_name| hidden_name == property_name)
                })
                .unwrap_or(false)
        });
        let enumerable = descriptor.enumerable.unwrap_or(current_enumerable);
        let value = if descriptor.is_accessor() {
            Expression::Undefined
        } else {
            descriptor
                .value
                .as_ref()
                .map(|expression| {
                    self.materialize_global_expression_with_state(
                        expression,
                        &HashMap::new(),
                        value_bindings,
                        object_bindings,
                    )
                    .unwrap_or_else(|| self.materialize_global_expression(expression))
                })
                .or(existing_value)
                .unwrap_or(Expression::Undefined)
        };
        let object_binding = object_bindings
            .entry(name.clone())
            .or_insert_with(empty_object_value_binding);
        object_binding_define_property(object_binding, property, value, enumerable);
    }

    pub(in crate::backend::direct_wasm) fn register_callback_bindings_for_call_with_state(
        &self,
        callee: &Expression,
        arguments: &[CallArgument],
        aliases: &HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
        value_bindings: &HashMap<String, Expression>,
        object_state: &HashMap<String, ObjectValueBinding>,
        overwrite_existing: bool,
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
                let Some(call_arguments) = self
                    .expand_apply_parameter_call_arguments_from_expression_with_state(
                        &apply_expression,
                        value_bindings,
                        object_state,
                    )
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
        let Some(parameter_array_bindings) = array_bindings.get_mut(&called_function_name) else {
            return;
        };
        let Some(parameter_object_bindings) = object_bindings.get_mut(&called_function_name) else {
            return;
        };

        let mut register_candidate =
            |param_name: &str, candidate: Option<LocalFunctionBinding>| match candidate {
                None => {
                    if overwrite_existing {
                        parameter_bindings.insert(param_name.to_string(), None);
                    } else {
                        parameter_bindings.insert(param_name.to_string(), None);
                    }
                }
                Some(binding) => match parameter_bindings.get(param_name) {
                    Some(None) if !overwrite_existing => {}
                    Some(Some(existing)) if *existing == binding => {}
                    Some(Some(_)) if !overwrite_existing => {
                        parameter_bindings.insert(param_name.to_string(), None);
                    }
                    _ => {
                        parameter_bindings.insert(param_name.to_string(), Some(binding));
                    }
                },
            };
        let mut register_object_candidate =
            |param_name: &str, candidate: Option<ObjectValueBinding>| match candidate {
                None if overwrite_existing => {
                    parameter_object_bindings.insert(param_name.to_string(), None);
                }
                None => {
                    parameter_object_bindings.insert(param_name.to_string(), None);
                }
                Some(binding) => match parameter_object_bindings.get(param_name) {
                    Some(None) if !overwrite_existing => {}
                    Some(Some(existing)) if *existing == binding => {}
                    Some(Some(_)) if !overwrite_existing => {
                        parameter_object_bindings.insert(param_name.to_string(), None);
                    }
                    _ => {
                        parameter_object_bindings.insert(param_name.to_string(), Some(binding));
                    }
                },
            };
        let mut register_array_candidate =
            |param_name: &str, candidate: Option<ArrayValueBinding>| match candidate {
                None => {
                    if overwrite_existing {
                        parameter_array_bindings.insert(param_name.to_string(), None);
                    } else {
                        parameter_array_bindings.insert(param_name.to_string(), None);
                    }
                }
                Some(binding) => match parameter_array_bindings.get(param_name) {
                    Some(None) if !overwrite_existing => {}
                    Some(Some(existing)) if *existing == binding => {}
                    Some(Some(_)) if !overwrite_existing => {
                        parameter_array_bindings.insert(param_name.to_string(), None);
                    }
                    _ => {
                        parameter_array_bindings.insert(param_name.to_string(), Some(binding));
                    }
                },
            };

        for (index, argument) in call_arguments.iter().enumerate() {
            if index >= user_function.params.len() {
                break;
            }
            let param_name = &user_function.params[index];
            register_candidate(
                param_name,
                self.resolve_function_binding_from_expression_with_aliases(argument, aliases),
            );
            let materialized_argument = self
                .materialize_global_expression_with_state(
                    argument,
                    &HashMap::new(),
                    value_bindings,
                    object_state,
                )
                .unwrap_or_else(|| self.materialize_global_expression(argument));
            register_array_candidate(
                param_name,
                self.infer_global_array_binding(&materialized_argument),
            );
            let mut value_state = value_bindings.clone();
            let mut object_state = object_state.clone();
            register_object_candidate(
                param_name,
                self.infer_global_object_binding_with_state(
                    argument,
                    &mut value_state,
                    &mut object_state,
                ),
            );
        }

        if call_arguments.len() < user_function.params.len() {
            for param_name in user_function.params.iter().skip(call_arguments.len()) {
                parameter_bindings.insert(param_name.to_string(), None);
                parameter_array_bindings.insert(param_name.to_string(), None);
                parameter_object_bindings.insert(param_name.to_string(), None);
            }
        }
    }

    pub(in crate::backend::direct_wasm) fn expand_apply_parameter_call_arguments_from_expression_with_state(
        &self,
        expression: &Expression,
        value_bindings: &HashMap<String, Expression>,
        object_bindings: &HashMap<String, ObjectValueBinding>,
    ) -> Option<Vec<Expression>> {
        let materialized = self
            .materialize_global_expression_with_state(
                expression,
                &HashMap::new(),
                value_bindings,
                object_bindings,
            )
            .unwrap_or_else(|| self.materialize_global_expression(expression));
        match &materialized {
            Expression::Undefined | Expression::Null => Some(Vec::new()),
            Expression::Array(elements) => {
                let mut value_bindings = value_bindings.clone();
                let mut object_bindings = object_bindings.clone();
                let mut values = Vec::new();
                for element in elements {
                    match element {
                        ArrayElement::Expression(expression) => {
                            if let Some(object_binding) = self
                                .infer_global_object_binding_with_state(
                                    expression,
                                    &mut value_bindings,
                                    &mut object_bindings,
                                )
                            {
                                values.push(object_binding_to_expression(&object_binding));
                            } else {
                                values.push(
                                    self.materialize_global_expression_with_state(
                                        expression,
                                        &HashMap::new(),
                                        &value_bindings,
                                        &object_bindings,
                                    )
                                    .unwrap_or_else(|| {
                                        self.materialize_global_expression(expression)
                                    }),
                                );
                            }
                        }
                        ArrayElement::Spread(expression) => {
                            let spread_expression = self
                                .materialize_global_expression_with_state(
                                    expression,
                                    &HashMap::new(),
                                    &value_bindings,
                                    &object_bindings,
                                )
                                .unwrap_or_else(|| self.materialize_global_expression(expression));
                            let array_binding =
                                self.infer_global_array_binding(&spread_expression)?;
                            values.extend(
                                array_binding
                                    .values
                                    .into_iter()
                                    .map(|value| value.unwrap_or(Expression::Undefined)),
                            );
                        }
                    }
                }
                Some(values)
            }
            _ => self.expand_apply_parameter_call_arguments_from_expression(&materialized),
        }
    }

    pub(in crate::backend::direct_wasm) fn register_callback_bindings_for_call(
        &self,
        callee: &Expression,
        arguments: &[CallArgument],
        aliases: &HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
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
        let Some(parameter_array_bindings) = array_bindings.get_mut(&called_function_name) else {
            return;
        };
        let Some(parameter_object_bindings) = object_bindings.get_mut(&called_function_name) else {
            return;
        };

        let mut register_candidate =
            |param_name: &str, candidate: Option<LocalFunctionBinding>| match candidate {
                None => {
                    parameter_bindings.insert(param_name.to_string(), None);
                }
                Some(binding) => match parameter_bindings.get(param_name) {
                    Some(None) => {}
                    Some(Some(existing)) if *existing == binding => {}
                    Some(Some(_)) => {
                        parameter_bindings.insert(param_name.to_string(), None);
                    }
                    None => {
                        parameter_bindings.insert(param_name.to_string(), Some(binding));
                    }
                },
            };
        let mut register_object_candidate =
            |param_name: &str, candidate: Option<ObjectValueBinding>| match candidate {
                None => {
                    parameter_object_bindings.insert(param_name.to_string(), None);
                }
                Some(binding) => match parameter_object_bindings.get(param_name) {
                    Some(None) => {}
                    Some(Some(existing)) if *existing == binding => {}
                    Some(Some(_)) => {
                        parameter_object_bindings.insert(param_name.to_string(), None);
                    }
                    None => {
                        parameter_object_bindings.insert(param_name.to_string(), Some(binding));
                    }
                },
            };
        let mut register_array_candidate =
            |param_name: &str, candidate: Option<ArrayValueBinding>| match candidate {
                None => {
                    parameter_array_bindings.insert(param_name.to_string(), None);
                }
                Some(binding) => match parameter_array_bindings.get(param_name) {
                    Some(None) => {}
                    Some(Some(existing)) if *existing == binding => {}
                    Some(Some(_)) => {
                        parameter_array_bindings.insert(param_name.to_string(), None);
                    }
                    None => {
                        parameter_array_bindings.insert(param_name.to_string(), Some(binding));
                    }
                },
            };

        for (index, argument) in call_arguments.iter().enumerate() {
            if index >= user_function.params.len() {
                break;
            }
            let param_name = &user_function.params[index];
            register_candidate(
                param_name,
                self.resolve_function_binding_from_expression_with_aliases(argument, aliases),
            );
            register_array_candidate(param_name, self.infer_global_array_binding(argument));
            register_object_candidate(param_name, self.infer_global_object_binding(argument));
        }

        if call_arguments.len() < user_function.params.len() {
            for param_name in user_function.params.iter().skip(call_arguments.len()) {
                parameter_bindings.insert(param_name.to_string(), None);
                parameter_array_bindings.insert(param_name.to_string(), None);
                parameter_object_bindings.insert(param_name.to_string(), None);
            }
        }
    }

    pub(in crate::backend::direct_wasm) fn expand_apply_parameter_call_arguments_from_expression(
        &self,
        expression: &Expression,
    ) -> Option<Vec<Expression>> {
        let materialized = self.materialize_global_expression(expression);
        match &materialized {
            Expression::Undefined | Expression::Null => Some(Vec::new()),
            Expression::Array(elements) => {
                let mut value_bindings = self.global_value_bindings.clone();
                let mut object_bindings = self.global_object_bindings.clone();
                let mut values = Vec::new();
                for element in elements {
                    match element {
                        ArrayElement::Expression(expression) => {
                            if let Some(object_binding) = self
                                .infer_global_object_binding_with_state(
                                    expression,
                                    &mut value_bindings,
                                    &mut object_bindings,
                                )
                            {
                                values.push(object_binding_to_expression(&object_binding));
                            } else {
                                values.push(
                                    self.materialize_global_expression_with_state(
                                        expression,
                                        &HashMap::new(),
                                        &value_bindings,
                                        &object_bindings,
                                    )
                                    .unwrap_or_else(|| {
                                        self.materialize_global_expression(expression)
                                    }),
                                );
                            }
                        }
                        ArrayElement::Spread(expression) => {
                            let spread_expression = self
                                .materialize_global_expression_with_state(
                                    expression,
                                    &HashMap::new(),
                                    &value_bindings,
                                    &object_bindings,
                                )
                                .unwrap_or_else(|| self.materialize_global_expression(expression));
                            let array_binding =
                                self.infer_global_array_binding(&spread_expression)?;
                            values.extend(
                                array_binding
                                    .values
                                    .into_iter()
                                    .map(|value| value.unwrap_or(Expression::Undefined)),
                            );
                        }
                    }
                }
                Some(values)
            }
            _ => {
                if let Some(array_binding) = self.infer_global_array_binding(&materialized) {
                    return Some(
                        array_binding
                            .values
                            .into_iter()
                            .map(|value| value.unwrap_or(Expression::Undefined))
                            .collect(),
                    );
                }
                self.infer_global_arguments_binding(&materialized)
                    .map(|binding| binding.values)
            }
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_function_binding_from_expression_with_aliases(
        &self,
        expression: &Expression,
        aliases: &HashMap<String, Option<LocalFunctionBinding>>,
    ) -> Option<LocalFunctionBinding> {
        match expression {
            Expression::Identifier(name) => {
                if let Some(function_binding) = aliases.get(name) {
                    return function_binding.clone();
                }
                if is_internal_user_function_identifier(name)
                    && self.user_function_map.contains_key(name)
                {
                    Some(LocalFunctionBinding::User(name.clone()))
                } else if let Some(function_binding) = self.global_function_bindings.get(name) {
                    Some(function_binding.clone())
                } else if name == "eval" || infer_call_result_kind(name).is_some() {
                    Some(LocalFunctionBinding::Builtin(name.clone()))
                } else {
                    None
                }
            }
            Expression::Sequence(expressions) => expressions.last().and_then(|expression| {
                self.resolve_function_binding_from_expression_with_aliases(expression, aliases)
            }),
            _ => None,
        }
    }
}
