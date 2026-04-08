use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_ordinary_to_primitive_plan(
        &self,
        expression: &Expression,
    ) -> Option<OrdinaryToPrimitivePlan> {
        let object_binding = self
            .resolve_object_binding_from_expression(expression)
            .or_else(|| self.resolve_effectful_returned_object_binding(expression))?;
        let mut steps = Vec::new();
        for method_name in ["valueOf", "toString"] {
            let property = Expression::String(method_name.to_string());
            let Some(method_value) = object_binding_lookup_value(&object_binding, &property) else {
                continue;
            };
            let binding = self.resolve_function_binding_from_expression(method_value)?;
            let outcome = self.resolve_terminal_function_outcome_from_binding(&binding, &[])?;
            steps.push(OrdinaryToPrimitiveStep { binding, outcome });
        }
        (!steps.is_empty()).then_some(OrdinaryToPrimitivePlan { steps })
    }

    pub(in crate::backend::direct_wasm) fn static_expression_is_non_object_primitive(
        &self,
        expression: &Expression,
    ) -> Option<bool> {
        match self.infer_value_kind(expression)? {
            StaticValueKind::Number
            | StaticValueKind::BigInt
            | StaticValueKind::String
            | StaticValueKind::Bool
            | StaticValueKind::Null
            | StaticValueKind::Undefined
            | StaticValueKind::Symbol => Some(true),
            StaticValueKind::Object | StaticValueKind::Function => Some(false),
            StaticValueKind::Unknown => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn analyze_ordinary_to_primitive_plan(
        &self,
        plan: &OrdinaryToPrimitivePlan,
    ) -> OrdinaryToPrimitiveAnalysis {
        for step in &plan.steps {
            match &step.outcome {
                StaticEvalOutcome::Throw(_) => return OrdinaryToPrimitiveAnalysis::Throw,
                StaticEvalOutcome::Value(value) => {
                    match self.static_expression_is_non_object_primitive(value) {
                        Some(true) => {
                            if let Some(kind) = self.infer_value_kind(value) {
                                return OrdinaryToPrimitiveAnalysis::Primitive(kind);
                            }
                            return OrdinaryToPrimitiveAnalysis::Unknown;
                        }
                        Some(false) => continue,
                        None => return OrdinaryToPrimitiveAnalysis::Unknown,
                    }
                }
            }
        }
        OrdinaryToPrimitiveAnalysis::TypeError
    }

    pub(in crate::backend::direct_wasm) fn emit_ordinary_to_primitive_from_plan(
        &mut self,
        expression: &Expression,
        plan: &OrdinaryToPrimitivePlan,
        result_local: u32,
    ) -> DirectResult<SymbolToPrimitiveHandling> {
        for step in &plan.steps {
            if !self.emit_binding_call_result_to_local_with_explicit_this(
                &step.binding,
                &[],
                expression,
                JS_TYPEOF_OBJECT_TAG,
                result_local,
            )? {
                return Ok(SymbolToPrimitiveHandling::NotHandled);
            }
            match &step.outcome {
                StaticEvalOutcome::Throw(_) => return Ok(SymbolToPrimitiveHandling::AlwaysThrows),
                StaticEvalOutcome::Value(value) => {
                    match self.static_expression_is_non_object_primitive(value) {
                        Some(true) => return Ok(SymbolToPrimitiveHandling::Handled),
                        Some(false) => continue,
                        None => return Ok(SymbolToPrimitiveHandling::NotHandled),
                    }
                }
            }
        }
        self.emit_named_error_throw("TypeError")?;
        Ok(SymbolToPrimitiveHandling::AlwaysThrows)
    }

    pub(in crate::backend::direct_wasm) fn emit_effectful_ordinary_to_primitive_addition(
        &mut self,
        left: &Expression,
        right: &Expression,
    ) -> DirectResult<bool> {
        let left_plan = self.resolve_ordinary_to_primitive_plan(left);
        let right_plan = self.resolve_ordinary_to_primitive_plan(right);
        let left_eval_throw = matches!(
            self.resolve_terminal_call_expression_outcome(left),
            Some(StaticEvalOutcome::Throw(_))
        );
        let right_eval_throw = matches!(
            self.resolve_terminal_call_expression_outcome(right),
            Some(StaticEvalOutcome::Throw(_))
        );
        let left_analysis = left_plan
            .as_ref()
            .map(|plan| self.analyze_ordinary_to_primitive_plan(plan))
            .unwrap_or(OrdinaryToPrimitiveAnalysis::Unknown);
        let right_analysis = right_plan
            .as_ref()
            .map(|plan| self.analyze_ordinary_to_primitive_plan(plan))
            .unwrap_or(OrdinaryToPrimitiveAnalysis::Unknown);

        let final_type_error = matches!(
            (left_analysis, right_analysis),
            (
                OrdinaryToPrimitiveAnalysis::Primitive(StaticValueKind::Symbol),
                _
            ) | (
                _,
                OrdinaryToPrimitiveAnalysis::Primitive(StaticValueKind::Symbol)
            ) | (OrdinaryToPrimitiveAnalysis::TypeError, _)
                | (_, OrdinaryToPrimitiveAnalysis::TypeError)
        );

        if !(left_eval_throw
            || right_eval_throw
            || matches!(left_analysis, OrdinaryToPrimitiveAnalysis::Throw)
            || matches!(right_analysis, OrdinaryToPrimitiveAnalysis::Throw)
            || final_type_error)
        {
            return Ok(false);
        }

        let left_local = self.allocate_temp_local();
        self.emit_numeric_expression(left)?;
        self.push_local_set(left_local);
        if left_eval_throw {
            self.push_i32_const(JS_UNDEFINED_TAG);
            return Ok(true);
        }

        let right_local = self.allocate_temp_local();
        self.emit_numeric_expression(right)?;
        self.push_local_set(right_local);
        if right_eval_throw {
            self.push_i32_const(JS_UNDEFINED_TAG);
            return Ok(true);
        }

        if let Some(plan) = left_plan.as_ref() {
            match self.emit_ordinary_to_primitive_from_plan(left, plan, left_local)? {
                SymbolToPrimitiveHandling::AlwaysThrows => {
                    self.push_i32_const(JS_UNDEFINED_TAG);
                    return Ok(true);
                }
                SymbolToPrimitiveHandling::Handled => {}
                SymbolToPrimitiveHandling::NotHandled => return Ok(false),
            }
        }

        if let Some(plan) = right_plan.as_ref() {
            match self.emit_ordinary_to_primitive_from_plan(right, plan, right_local)? {
                SymbolToPrimitiveHandling::AlwaysThrows => {
                    self.push_i32_const(JS_UNDEFINED_TAG);
                    return Ok(true);
                }
                SymbolToPrimitiveHandling::Handled => {}
                SymbolToPrimitiveHandling::NotHandled => return Ok(false),
            }
        }

        if final_type_error {
            self.emit_named_error_throw("TypeError")?;
            self.push_i32_const(JS_UNDEFINED_TAG);
            return Ok(true);
        }

        Ok(true)
    }
}
