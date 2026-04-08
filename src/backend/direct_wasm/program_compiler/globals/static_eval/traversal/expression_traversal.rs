use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn register_static_eval_functions_in_expression(
        &mut self,
        expression: &Expression,
        current_function_name: Option<&str>,
    ) -> DirectResult<()> {
        match expression {
            Expression::Call { callee, arguments } => {
                if let Some(CallArgument::Expression(Expression::String(source))) =
                    arguments.first()
                {
                    if matches!(callee.as_ref(), Expression::Identifier(name) if name == "eval")
                        && let Some(eval_program) =
                            self.parse_static_eval_program_in_context(source, current_function_name)
                    {
                        let mut eval_program = eval_program;
                        namespace_eval_program_internal_function_names(
                            &mut eval_program,
                            current_function_name,
                            source,
                        );
                        self.register_eval_local_function_bindings(
                            current_function_name,
                            &eval_program,
                        );
                        let new_functions = eval_program
                            .functions
                            .iter()
                            .filter(|function| !self.contains_user_function(&function.name))
                            .cloned()
                            .collect::<Vec<_>>();
                        if !new_functions.is_empty() {
                            self.register_functions(&new_functions)?;
                        }
                        let global_functions = eval_program
                            .functions
                            .iter()
                            .filter(|function| function.register_global)
                            .cloned()
                            .collect::<Vec<_>>();
                        if !global_functions.is_empty() {
                            self.register_functions(&global_functions)?;
                            for function in &global_functions {
                                self.ensure_implicit_global_binding(&function.name);
                                self.set_global_user_function_reference(&function.name);
                            }
                        }
                        self.register_static_eval_functions(&eval_program)?;
                    } else if matches!(
                        callee.as_ref(),
                        Expression::Sequence(expressions)
                            if matches!(expressions.last(), Some(Expression::Identifier(name)) if name == "eval")
                    ) && let Ok(mut eval_program) = frontend::parse(source)
                    {
                        namespace_eval_program_internal_function_names(
                            &mut eval_program,
                            current_function_name,
                            source,
                        );
                        if !eval_program.strict {
                            for name in collect_eval_var_names(&eval_program) {
                                if self.global_has_binding(&name) {
                                    continue;
                                }
                                self.ensure_implicit_global_binding(&name);
                            }
                        }
                        let new_functions = eval_program
                            .functions
                            .iter()
                            .filter(|function| !self.contains_user_function(&function.name))
                            .cloned()
                            .collect::<Vec<_>>();
                        if !new_functions.is_empty() {
                            self.register_functions(&new_functions)?;
                        }
                        self.register_static_eval_functions(&eval_program)?;
                    }
                }
                self.register_static_eval_functions_in_expression(callee, current_function_name)?;
                for argument in arguments {
                    match argument {
                        CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                            self.register_static_eval_functions_in_expression(
                                expression,
                                current_function_name,
                            )?;
                        }
                    }
                }
            }
            Expression::Array(elements) => {
                for element in elements {
                    match element {
                        ArrayElement::Expression(expression) | ArrayElement::Spread(expression) => {
                            self.register_static_eval_functions_in_expression(
                                expression,
                                current_function_name,
                            )?;
                        }
                    }
                }
            }
            Expression::Object(entries) => {
                for entry in entries {
                    match entry {
                        ObjectEntry::Data { key, value } => {
                            self.register_static_eval_functions_in_expression(
                                key,
                                current_function_name,
                            )?;
                            self.register_static_eval_functions_in_expression(
                                value,
                                current_function_name,
                            )?;
                        }
                        ObjectEntry::Getter { key, getter } => {
                            self.register_static_eval_functions_in_expression(
                                key,
                                current_function_name,
                            )?;
                            self.register_static_eval_functions_in_expression(
                                getter,
                                current_function_name,
                            )?;
                        }
                        ObjectEntry::Setter { key, setter } => {
                            self.register_static_eval_functions_in_expression(
                                key,
                                current_function_name,
                            )?;
                            self.register_static_eval_functions_in_expression(
                                setter,
                                current_function_name,
                            )?;
                        }
                        ObjectEntry::Spread(expression) => {
                            self.register_static_eval_functions_in_expression(
                                expression,
                                current_function_name,
                            )?;
                        }
                    }
                }
            }
            Expression::Member { object, property } => {
                self.register_static_eval_functions_in_expression(object, current_function_name)?;
                self.register_static_eval_functions_in_expression(property, current_function_name)?;
            }
            Expression::Assign { value, .. }
            | Expression::Await(value)
            | Expression::EnumerateKeys(value)
            | Expression::GetIterator(value)
            | Expression::IteratorClose(value)
            | Expression::Unary {
                expression: value, ..
            } => {
                self.register_static_eval_functions_in_expression(value, current_function_name)?;
            }
            Expression::AssignMember {
                object,
                property,
                value,
            } => {
                self.register_static_eval_functions_in_expression(object, current_function_name)?;
                self.register_static_eval_functions_in_expression(property, current_function_name)?;
                self.register_static_eval_functions_in_expression(value, current_function_name)?;
            }
            Expression::AssignSuperMember { property, value } => {
                self.register_static_eval_functions_in_expression(property, current_function_name)?;
                self.register_static_eval_functions_in_expression(value, current_function_name)?;
            }
            Expression::Binary { left, right, .. } => {
                self.register_static_eval_functions_in_expression(left, current_function_name)?;
                self.register_static_eval_functions_in_expression(right, current_function_name)?;
            }
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                self.register_static_eval_functions_in_expression(
                    condition,
                    current_function_name,
                )?;
                self.register_static_eval_functions_in_expression(
                    then_expression,
                    current_function_name,
                )?;
                self.register_static_eval_functions_in_expression(
                    else_expression,
                    current_function_name,
                )?;
            }
            Expression::Sequence(expressions) => {
                for expression in expressions {
                    self.register_static_eval_functions_in_expression(
                        expression,
                        current_function_name,
                    )?;
                }
            }
            Expression::SuperCall { callee, arguments } | Expression::New { callee, arguments } => {
                self.register_static_eval_functions_in_expression(callee, current_function_name)?;
                for argument in arguments {
                    match argument {
                        CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                            self.register_static_eval_functions_in_expression(
                                expression,
                                current_function_name,
                            )?;
                        }
                    }
                }
            }
            Expression::SuperMember { property } => {
                self.register_static_eval_functions_in_expression(property, current_function_name)?;
            }
            Expression::Number(_)
            | Expression::BigInt(_)
            | Expression::String(_)
            | Expression::Bool(_)
            | Expression::Null
            | Expression::Undefined
            | Expression::Identifier(_)
            | Expression::Update { .. }
            | Expression::NewTarget
            | Expression::This
            | Expression::Sent => {}
        }
        Ok(())
    }
}
