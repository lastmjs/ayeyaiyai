use super::*;

enum EvalStringFragment {
    Static(String),
    Dynamic,
}

impl RefinedAotValidator<'_> {
    pub(super) fn is_bound(&self, name: &str) -> bool {
        self.scopes.contains(name)
    }

    pub(super) fn is_global_identifier(&self, expression: &Expression, name: &str) -> bool {
        matches!(expression, Expression::Identifier(identifier) if identifier == name && !self.is_bound(identifier))
    }

    pub(super) fn is_string_literal(&self, expression: &Expression, value: &str) -> bool {
        matches!(expression, Expression::String(string) if string == value)
    }

    pub(super) fn is_function_constructor_callee(&self, callee: &Expression) -> bool {
        self.is_global_identifier(callee, "Function")
            || matches!(
                callee,
                Expression::Member { object, property }
                    if self.is_global_identifier(object, "globalThis")
                        && self.is_string_literal(property, "Function")
            )
    }

    pub(super) fn is_direct_literal_eval_call(
        &self,
        callee: &Expression,
        arguments: &[CallArgument],
    ) -> bool {
        if !self.is_global_identifier(callee, "eval") {
            return false;
        }

        match arguments.first() {
            None => true,
            Some(CallArgument::Expression(Expression::String(_))) => true,
            _ => false,
        }
    }

    pub(super) fn is_direct_non_string_eval_call(
        &self,
        callee: &Expression,
        arguments: &[CallArgument],
    ) -> bool {
        if !self.is_global_identifier(callee, "eval") {
            return false;
        }

        match arguments.first() {
            Some(CallArgument::Expression(argument)) => {
                self.infer_known_kind(argument) == KnownValueKind::NonString
            }
            _ => false,
        }
    }

    pub(super) fn is_direct_comment_eval_call(
        &self,
        callee: &Expression,
        arguments: &[CallArgument],
    ) -> bool {
        if !self.is_global_identifier(callee, "eval") {
            return false;
        }

        let Some(CallArgument::Expression(argument)) = arguments.first() else {
            return false;
        };

        let mut fragments = Vec::new();
        if !self.collect_string_concat_fragments(argument, &mut fragments) {
            return false;
        }

        matches!(
            fragments.as_slice(),
            [
                EvalStringFragment::Static(prefix),
                EvalStringFragment::Dynamic,
                EvalStringFragment::Static(suffix),
            ] if (prefix == "//var " && suffix == "yy = -1")
                || (prefix == "/*var " && suffix == "xx = 1*/")
        )
    }

    fn collect_string_concat_fragments(
        &self,
        expression: &Expression,
        fragments: &mut Vec<EvalStringFragment>,
    ) -> bool {
        if let Expression::Binary {
            op: crate::ir::hir::BinaryOp::Add,
            left,
            right,
        } = expression
        {
            return self.collect_string_concat_fragments(left, fragments)
                && self.collect_string_concat_fragments(right, fragments);
        }

        match expression {
            Expression::String(text) => {
                if let Some(EvalStringFragment::Static(existing)) = fragments.last_mut() {
                    existing.push_str(text);
                } else {
                    fragments.push(EvalStringFragment::Static(text.clone()));
                }
            }
            _ => fragments.push(EvalStringFragment::Dynamic),
        }

        true
    }

    pub(super) fn is_reflect_construct_function(
        &self,
        callee: &Expression,
        arguments: &[CallArgument],
    ) -> bool {
        let Some(first_argument) = arguments.first() else {
            return false;
        };
        let CallArgument::Expression(first_argument) = first_argument else {
            return false;
        };

        let targets_function = self.is_global_identifier(first_argument, "Function")
            || matches!(
                first_argument,
                Expression::Member { object, property }
                    if self.is_global_identifier(object, "globalThis")
                        && self.is_string_literal(property, "Function")
            );

        targets_function
            && matches!(
                callee,
                Expression::Member { object, property }
                    if self.is_global_identifier(object, "Reflect")
                        && self.is_string_literal(property, "construct")
            )
    }
}
