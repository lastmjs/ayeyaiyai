use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn conditional_defined_binding_narrowing(
        &self,
        condition: &Expression,
        then_branch: bool,
    ) -> Option<(String, Expression)> {
        let (name, defined_when_condition_true) = match condition {
            Expression::Binary {
                op: BinaryOp::NotEqual,
                left,
                right,
            } if matches!(right.as_ref(), Expression::Undefined) => {
                let Expression::Identifier(name) = left.as_ref() else {
                    return None;
                };
                (name.clone(), true)
            }
            Expression::Binary {
                op: BinaryOp::NotEqual,
                left,
                right,
            } if matches!(left.as_ref(), Expression::Undefined) => {
                let Expression::Identifier(name) = right.as_ref() else {
                    return None;
                };
                (name.clone(), true)
            }
            Expression::Binary {
                op: BinaryOp::Equal,
                left,
                right,
            } if matches!(right.as_ref(), Expression::Undefined) => {
                let Expression::Identifier(name) = left.as_ref() else {
                    return None;
                };
                (name.clone(), false)
            }
            Expression::Binary {
                op: BinaryOp::Equal,
                left,
                right,
            } if matches!(left.as_ref(), Expression::Undefined) => {
                let Expression::Identifier(name) = right.as_ref() else {
                    return None;
                };
                (name.clone(), false)
            }
            _ => return None,
        };

        let Expression::Conditional {
            then_expression,
            else_expression,
            ..
        } = self
            .state
            .speculation
            .static_semantics
            .local_value_binding(&name)?
        else {
            return None;
        };

        let then_is_undefined = matches!(then_expression.as_ref(), Expression::Undefined);
        let else_is_undefined = matches!(else_expression.as_ref(), Expression::Undefined);
        if then_is_undefined == else_is_undefined {
            return None;
        }

        let defined_expression = if !then_is_undefined {
            then_expression.as_ref().clone()
        } else {
            else_expression.as_ref().clone()
        };
        let branch_expression = if then_branch == defined_when_condition_true {
            defined_expression
        } else {
            Expression::Undefined
        };
        Some((name, branch_expression))
    }

    pub(in crate::backend::direct_wasm) fn with_restored_static_binding_metadata<T>(
        &mut self,
        callback: impl FnOnce(&mut Self) -> DirectResult<T>,
    ) -> DirectResult<T> {
        let transaction = StaticBindingMetadataTransaction::capture(self);

        let result = callback(self);

        transaction.restore(self);

        result
    }

    pub(in crate::backend::direct_wasm) fn with_restored_function_static_binding_metadata<T>(
        &mut self,
        callback: impl FnOnce(&mut Self) -> DirectResult<T>,
    ) -> DirectResult<T> {
        let transaction = FunctionStaticBindingMetadataTransaction::capture(&self.state);

        let result = callback(self);

        transaction.restore(&mut self.state);

        result
    }

    pub(in crate::backend::direct_wasm) fn with_narrowed_local_binding_metadata<T>(
        &mut self,
        name: &str,
        expression: &Expression,
        callback: impl FnOnce(&mut Self) -> DirectResult<T>,
    ) -> DirectResult<T> {
        let saved_binding = self.state.snapshot_local_static_binding(name);
        let array_binding = self.resolve_array_binding_from_expression(expression);
        let object_binding = self.resolve_object_binding_from_expression(expression);
        let kind = self.infer_value_kind(expression);
        self.state.set_local_static_binding(
            name,
            expression.clone(),
            array_binding,
            object_binding,
            kind,
        );

        let result = callback(self);

        self.state.restore_local_static_binding(saved_binding);

        result
    }

    pub(in crate::backend::direct_wasm) fn invalidate_static_binding_metadata_for_names(
        &mut self,
        names: &HashSet<String>,
    ) {
        for name in names {
            self.clear_static_identifier_binding_metadata(name);
        }
    }

    pub(in crate::backend::direct_wasm) fn invalidate_static_binding_metadata_for_names_with_preserved_kinds(
        &mut self,
        names: &HashSet<String>,
        preserved_kinds: &HashMap<String, StaticValueKind>,
    ) {
        self.invalidate_static_binding_metadata_for_names(names);
        for (name, kind) in preserved_kinds {
            if let Some((resolved_name, _)) = self.resolve_current_local_binding(name) {
                self.state
                    .speculation
                    .static_semantics
                    .set_local_kind(&resolved_name, *kind);
            } else if self.state.runtime.locals.bindings.contains_key(name)
                || self.parameter_scope_arguments_local_for(name).is_some()
            {
                self.state
                    .speculation
                    .static_semantics
                    .set_local_kind(name, *kind);
            } else if let Some(hidden_name) = self.resolve_user_function_capture_hidden_name(name) {
                self.backend.set_global_binding_kind(&hidden_name, *kind);
            } else if self.binding_name_is_global(name) || self.backend.global_has_binding(name) {
                self.backend.set_global_binding_kind(name, *kind);
            } else {
                self.state
                    .speculation
                    .static_semantics
                    .set_local_kind(name, *kind);
            }
        }
    }
}
