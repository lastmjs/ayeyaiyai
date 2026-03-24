use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn collect_string_concat_fragments(
        &self,
        expression: &Expression,
        fragments: &mut Vec<StringConcatFragment>,
    ) -> bool {
        if let Some(resolved) = self.resolve_bound_alias_expression(expression) {
            if !static_expression_matches(&resolved, expression) {
                if self
                    .resolve_single_char_code_expression(&resolved)
                    .is_some()
                {
                    fragments.push(StringConcatFragment::Dynamic(resolved));
                    return true;
                }
                return self.collect_string_concat_fragments(&resolved, fragments);
            }
        }

        if self
            .resolve_single_char_code_expression(expression)
            .is_some()
        {
            fragments.push(StringConcatFragment::Dynamic(expression.clone()));
            return true;
        }

        if let Expression::Binary {
            op: BinaryOp::Add,
            left,
            right,
        } = expression
        {
            return self.collect_string_concat_fragments(left, fragments)
                && self.collect_string_concat_fragments(right, fragments);
        }

        if let Some(text) = self.resolve_static_string_value(expression) {
            if let Some(StringConcatFragment::Static(existing)) = fragments.last_mut() {
                existing.push_str(&text);
            } else {
                fragments.push(StringConcatFragment::Static(text));
            }
            return true;
        }

        fragments.push(StringConcatFragment::Dynamic(expression.clone()));
        true
    }
}
