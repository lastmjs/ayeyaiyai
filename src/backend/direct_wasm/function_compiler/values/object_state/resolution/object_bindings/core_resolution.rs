use super::*;

mod basic_resolution;
mod call_resolution;
mod iterator_resolution;
mod member_resolution;
mod object_literals;

thread_local! {
    static OBJECT_BINDING_RESOLUTION_STACK: std::cell::RefCell<Vec<Expression>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

impl<'a> FunctionCompiler<'a> {
    fn with_object_binding_resolution_guard<T>(
        &self,
        expression: &Expression,
        f: impl FnOnce(&Self) -> Option<T>,
    ) -> Option<T> {
        let reentered = OBJECT_BINDING_RESOLUTION_STACK.with(|stack| {
            stack
                .borrow()
                .iter()
                .any(|visited| static_expression_matches(visited, expression))
        });
        if reentered {
            return None;
        }

        OBJECT_BINDING_RESOLUTION_STACK.with(|stack| {
            stack.borrow_mut().push(expression.clone());
        });
        let result = f(self);
        OBJECT_BINDING_RESOLUTION_STACK.with(|stack| {
            stack.borrow_mut().pop();
        });
        result
    }

    pub(crate) fn object_binding_from_property_descriptor(
        &self,
        descriptor: &PropertyDescriptorBinding,
    ) -> ObjectValueBinding {
        let mut object_binding = empty_object_value_binding();
        object_binding_set_property(
            &mut object_binding,
            Expression::String("configurable".to_string()),
            Expression::Bool(descriptor.configurable),
        );
        object_binding_set_property(
            &mut object_binding,
            Expression::String("enumerable".to_string()),
            Expression::Bool(descriptor.enumerable),
        );
        if let Some(value) = descriptor.value.clone() {
            object_binding_set_property(
                &mut object_binding,
                Expression::String("value".to_string()),
                value,
            );
        }
        if let Some(writable) = descriptor.writable {
            object_binding_set_property(
                &mut object_binding,
                Expression::String("writable".to_string()),
                Expression::Bool(writable),
            );
        }
        if descriptor.has_get {
            object_binding_set_property(
                &mut object_binding,
                Expression::String("get".to_string()),
                descriptor.getter.clone().unwrap_or(Expression::Undefined),
            );
        }
        if descriptor.has_set {
            object_binding_set_property(
                &mut object_binding,
                Expression::String("set".to_string()),
                descriptor.setter.clone().unwrap_or(Expression::Undefined),
            );
        }
        object_binding
    }

    pub(in crate::backend::direct_wasm) fn expression_uses_runtime_dynamic_binding(
        &self,
        expression: &Expression,
    ) -> bool {
        if let Expression::Identifier(name) = expression
            && self
                .state
                .runtime
                .locals
                .runtime_dynamic_bindings
                .contains(name)
        {
            return true;
        }
        self.resolve_bound_alias_expression(expression)
            .is_some_and(|resolved| {
                matches!(resolved, Expression::Identifier(name)
                    if self
                        .state
                        .runtime
                        .locals
                        .runtime_dynamic_bindings
                        .contains(&name))
            })
    }

    fn resolve_descriptor_object_binding(
        &self,
        expression: &Expression,
    ) -> Option<ObjectValueBinding> {
        self.resolve_descriptor_binding_from_expression(expression)
            .map(|descriptor| self.object_binding_from_property_descriptor(&descriptor))
    }

    fn resolve_fallback_object_binding(
        &self,
        expression: &Expression,
    ) -> Option<ObjectValueBinding> {
        self.resolve_bound_alias_expression(expression)
            .filter(|resolved| resolved != expression)
            .and_then(|resolved| self.resolve_object_binding_from_expression(&resolved))
            .or_else(|| {
                let materialized = self.materialize_static_expression(expression);
                (!static_expression_matches(&materialized, expression))
                    .then(|| self.resolve_object_binding_from_expression(&materialized))
                    .flatten()
            })
    }

    pub(in crate::backend::direct_wasm) fn resolve_object_binding_from_expression(
        &self,
        expression: &Expression,
    ) -> Option<ObjectValueBinding> {
        self.with_object_binding_resolution_guard(expression, |this| {
            this.resolve_descriptor_object_binding(expression)
                .or_else(|| this.resolve_basic_object_binding(expression))
                .or_else(|| this.resolve_member_object_binding(expression))
                .or_else(|| this.resolve_iterator_expression_object_binding(expression))
                .or_else(|| this.resolve_call_or_new_object_binding(expression))
                .or_else(|| this.resolve_object_literal_expression_binding(expression))
                .or_else(|| this.resolve_fallback_object_binding(expression))
        })
    }
}
