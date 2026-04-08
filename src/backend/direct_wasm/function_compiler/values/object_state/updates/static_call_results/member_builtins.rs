use super::*;

fn simple_regexp_pattern_is_plain_literal(pattern: &str) -> bool {
    !pattern.chars().any(|character| {
        matches!(
            character,
            '\\' | '.' | '^' | '$' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|'
        )
    })
}

impl<'a> FunctionCompiler<'a> {
    pub(super) fn resolve_static_member_builtin_call_result_with_context(
        &self,
        callee: &Expression,
        arguments: &[CallArgument],
        current_function_name: Option<&str>,
    ) -> Option<(Expression, Option<String>)> {
        if let Expression::Member { object, property } = callee
            && matches!(property.as_ref(), Expression::String(name) if name == "replace")
            && let [
                CallArgument::Expression(search_expression),
                CallArgument::Expression(replacement_expression),
            ] = arguments
            && let Some(text) = self.resolve_static_string_replace_result_with_context(
                object,
                search_expression,
                replacement_expression,
                current_function_name,
            )
        {
            return Some((Expression::String(text), None));
        }

        if let Expression::Member { object, property } = callee
            && matches!(property.as_ref(), Expression::String(name) if name == "exec")
            && let [CallArgument::Expression(subject) | CallArgument::Spread(subject)] = arguments
            && let Some(true) = self.resolve_static_simple_regexp_exec_no_match(
                object,
                subject,
                current_function_name,
            )
        {
            return Some((Expression::Null, None));
        }

        if let Expression::Member { object, property } = callee
            && matches!(object.as_ref(), Expression::Identifier(name) if name == "Object")
            && matches!(property.as_ref(), Expression::String(name) if name == "getPrototypeOf")
            && let [CallArgument::Expression(target), ..] = arguments
            && let Some(prototype) = self.resolve_static_object_prototype_expression(target)
        {
            return Some((prototype, None));
        }

        if let Expression::Member { object, property } = callee
            && matches!(object.as_ref(), Expression::Identifier(name) if name == "Object")
            && matches!(property.as_ref(), Expression::String(name) if name == "isExtensible")
            && let [CallArgument::Expression(target), ..] = arguments
        {
            return Some((
                Expression::Bool(
                    self.resolve_static_object_prototype_expression(target)
                        .is_some(),
                ),
                None,
            ));
        }

        None
    }

    pub(in crate::backend::direct_wasm) fn resolve_static_simple_regexp_exec_no_match(
        &self,
        regexp_expression: &Expression,
        subject_expression: &Expression,
        current_function_name: Option<&str>,
    ) -> Option<bool> {
        let resolved = self
            .resolve_bound_alias_expression(regexp_expression)
            .unwrap_or_else(|| self.materialize_static_expression(regexp_expression));
        let (callee, arguments) = match resolved {
            Expression::Call { callee, arguments } | Expression::New { callee, arguments } => {
                (callee, arguments)
            }
            _ => return None,
        };
        let Expression::Identifier(name) = callee.as_ref() else {
            return None;
        };
        if name != "RegExp" || !self.is_unshadowed_builtin_identifier(name) {
            return None;
        }

        let pattern = match arguments.first() {
            Some(CallArgument::Expression(argument) | CallArgument::Spread(argument)) => {
                self.resolve_static_string_concat_value(argument, current_function_name)?
            }
            None => String::new(),
        };
        let flags = match arguments.get(1) {
            Some(CallArgument::Expression(argument) | CallArgument::Spread(argument)) => {
                self.resolve_static_string_concat_value(argument, current_function_name)?
            }
            None => String::new(),
        };
        if !flags.is_empty() || !simple_regexp_pattern_is_plain_literal(&pattern) {
            return None;
        }

        let subject =
            self.resolve_static_string_concat_value(subject_expression, current_function_name)?;
        Some(!subject.contains(&pattern))
    }
}
