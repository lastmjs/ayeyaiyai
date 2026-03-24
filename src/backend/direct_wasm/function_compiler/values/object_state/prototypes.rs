use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn with_scope_blocks_static_identifier_resolution(
        &self,
        name: &str,
    ) -> bool {
        !self.with_scopes.is_empty() && !name.starts_with("__ayy_")
    }

    pub(in crate::backend::direct_wasm) fn merge_object_binding_properties(
        target: &mut ObjectValueBinding,
        source: &ObjectValueBinding,
    ) {
        for (name, value) in &source.string_properties {
            let enumerable = !source
                .non_enumerable_string_properties
                .iter()
                .any(|hidden_name| hidden_name == name);
            object_binding_define_property(
                target,
                Expression::String(name.clone()),
                value.clone(),
                enumerable,
            );
        }
        for (property, value) in &source.symbol_properties {
            object_binding_define_property(target, property.clone(), value.clone(), true);
        }
    }

    pub(in crate::backend::direct_wasm) fn default_function_prototype_object_binding(
        &self,
        function_binding: &LocalFunctionBinding,
    ) -> Option<ObjectValueBinding> {
        let constructor_expression = match function_binding {
            LocalFunctionBinding::User(function_name) => {
                let user_function = self.module.user_function_map.get(function_name)?;
                if !user_function.is_constructible() {
                    return None;
                }
                Expression::Identifier(function_name.clone())
            }
            LocalFunctionBinding::Builtin(function_name) => {
                if !is_function_constructor_builtin(function_name) {
                    return None;
                }
                Expression::Identifier(function_name.clone())
            }
        };

        let mut object_binding = empty_object_value_binding();
        object_binding_define_property(
            &mut object_binding,
            Expression::String("constructor".to_string()),
            constructor_expression,
            false,
        );
        Some(object_binding)
    }

    pub(in crate::backend::direct_wasm) fn resolve_function_prototype_object_binding(
        &self,
        name: &str,
    ) -> Option<ObjectValueBinding> {
        let stored_binding = self
            .local_prototype_object_bindings
            .get(name)
            .cloned()
            .or_else(|| {
                self.module
                    .global_prototype_object_bindings
                    .get(name)
                    .cloned()
            });
        let default_binding = self
            .resolve_function_binding_from_expression(&Expression::Identifier(name.to_string()))
            .and_then(|binding| self.default_function_prototype_object_binding(&binding));

        match (default_binding, stored_binding) {
            (Some(mut default_binding), Some(stored_binding)) => {
                Self::merge_object_binding_properties(&mut default_binding, &stored_binding);
                Some(default_binding)
            }
            (Some(default_binding), None) => Some(default_binding),
            (None, Some(stored_binding)) => Some(stored_binding),
            (None, None) => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn substitute_user_function_statement_call_frame_bindings(
        &self,
        statement: &Statement,
        user_function: &UserFunction,
        arguments: &[CallArgument],
        this_binding: &Expression,
        arguments_binding: &Expression,
    ) -> Statement {
        match statement {
            Statement::Block { body } => Statement::Block {
                body: body
                    .iter()
                    .map(|statement| {
                        self.substitute_user_function_statement_call_frame_bindings(
                            statement,
                            user_function,
                            arguments,
                            this_binding,
                            arguments_binding,
                        )
                    })
                    .collect(),
            },
            Statement::Var { name, value } => Statement::Var {
                name: name.clone(),
                value: self.substitute_user_function_call_frame_bindings(
                    value,
                    user_function,
                    arguments,
                    this_binding,
                    arguments_binding,
                ),
            },
            Statement::Let {
                name,
                mutable,
                value,
            } => Statement::Let {
                name: name.clone(),
                mutable: *mutable,
                value: self.substitute_user_function_call_frame_bindings(
                    value,
                    user_function,
                    arguments,
                    this_binding,
                    arguments_binding,
                ),
            },
            Statement::Assign { name, value } => Statement::Assign {
                name: name.clone(),
                value: self.substitute_user_function_call_frame_bindings(
                    value,
                    user_function,
                    arguments,
                    this_binding,
                    arguments_binding,
                ),
            },
            Statement::AssignMember {
                object,
                property,
                value,
            } => Statement::AssignMember {
                object: self.substitute_user_function_call_frame_bindings(
                    object,
                    user_function,
                    arguments,
                    this_binding,
                    arguments_binding,
                ),
                property: self.substitute_user_function_call_frame_bindings(
                    property,
                    user_function,
                    arguments,
                    this_binding,
                    arguments_binding,
                ),
                value: self.substitute_user_function_call_frame_bindings(
                    value,
                    user_function,
                    arguments,
                    this_binding,
                    arguments_binding,
                ),
            },
            Statement::Print { values } => Statement::Print {
                values: values
                    .iter()
                    .map(|value| {
                        self.substitute_user_function_call_frame_bindings(
                            value,
                            user_function,
                            arguments,
                            this_binding,
                            arguments_binding,
                        )
                    })
                    .collect(),
            },
            Statement::Expression(expression) => {
                Statement::Expression(self.substitute_user_function_call_frame_bindings(
                    expression,
                    user_function,
                    arguments,
                    this_binding,
                    arguments_binding,
                ))
            }
            Statement::Throw(expression) => {
                Statement::Throw(self.substitute_user_function_call_frame_bindings(
                    expression,
                    user_function,
                    arguments,
                    this_binding,
                    arguments_binding,
                ))
            }
            Statement::Return(expression) => {
                Statement::Return(self.substitute_user_function_call_frame_bindings(
                    expression,
                    user_function,
                    arguments,
                    this_binding,
                    arguments_binding,
                ))
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => Statement::If {
                condition: self.substitute_user_function_call_frame_bindings(
                    condition,
                    user_function,
                    arguments,
                    this_binding,
                    arguments_binding,
                ),
                then_branch: then_branch
                    .iter()
                    .map(|statement| {
                        self.substitute_user_function_statement_call_frame_bindings(
                            statement,
                            user_function,
                            arguments,
                            this_binding,
                            arguments_binding,
                        )
                    })
                    .collect(),
                else_branch: else_branch
                    .iter()
                    .map(|statement| {
                        self.substitute_user_function_statement_call_frame_bindings(
                            statement,
                            user_function,
                            arguments,
                            this_binding,
                            arguments_binding,
                        )
                    })
                    .collect(),
            },
            _ => statement.clone(),
        }
    }

    pub(in crate::backend::direct_wasm) fn prototype_member_expression(name: &str) -> Expression {
        Expression::Member {
            object: Box::new(Expression::Identifier(name.to_string())),
            property: Box::new(Expression::String("prototype".to_string())),
        }
    }

    pub(in crate::backend::direct_wasm) fn builtin_constructor_object_prototype_expression(
        name: &str,
    ) -> Option<Expression> {
        if matches!(
            name,
            "AggregateError"
                | "EvalError"
                | "RangeError"
                | "ReferenceError"
                | "SyntaxError"
                | "TypeError"
                | "URIError"
        ) {
            return Some(Expression::Identifier("Error".to_string()));
        }
        if builtin_identifier_kind(name) == Some(StaticValueKind::Function)
            || infer_call_result_kind(name).is_some()
        {
            return Some(Self::prototype_member_expression("Function"));
        }
        None
    }

    pub(in crate::backend::direct_wasm) fn builtin_prototype_object_prototype_expression(
        name: &str,
    ) -> Option<Expression> {
        if name == "Object" {
            return Some(Expression::Null);
        }
        if matches!(
            name,
            "AggregateError"
                | "EvalError"
                | "RangeError"
                | "ReferenceError"
                | "SyntaxError"
                | "TypeError"
                | "URIError"
        ) {
            return Some(Self::prototype_member_expression("Error"));
        }
        if name == "Error"
            || builtin_identifier_kind(name) == Some(StaticValueKind::Function)
            || infer_call_result_kind(name).is_some()
        {
            return Some(Self::prototype_member_expression("Object"));
        }
        None
    }
}
