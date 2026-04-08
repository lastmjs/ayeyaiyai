use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_static_direct_eval_outcome(
        &self,
        arguments: &[CallArgument],
    ) -> Option<StaticEvalOutcome> {
        let CallArgument::Expression(Expression::String(argument_source)) = arguments.first()?
        else {
            return None;
        };

        let raw_source = argument_source.clone();
        let argument_source = if self.state.speculation.execution_context.strict_mode {
            let mut strict_argument_source = String::from("\"use strict\";");
            strict_argument_source.push_str(argument_source);
            Cow::Owned(strict_argument_source)
        } else {
            Cow::Borrowed(argument_source.as_str())
        };

        let mut program = if let Some(program) =
            self.parse_eval_program_in_current_function_context(&argument_source)
        {
            program
        } else if let Ok(program) = frontend::parse_script_goal(&argument_source) {
            program
        } else {
            return Some(StaticEvalOutcome::Throw(StaticThrowValue::NamedError(
                "SyntaxError",
            )));
        };

        namespace_eval_program_internal_function_names(
            &mut program,
            self.current_function_name(),
            &raw_source,
        );
        self.normalize_eval_scoped_bindings_to_source_names(&mut program);

        if self.eval_arguments_declaration_conflicts(&program)
            || self.eval_program_declares_var_collision_with_global_lexical(&program)
            || self.eval_program_declares_var_collision_with_active_lexical(&program)
        {
            return Some(StaticEvalOutcome::Throw(StaticThrowValue::NamedError(
                "SyntaxError",
            )));
        }

        if self.eval_program_declares_non_definable_global_function(&program) {
            return Some(StaticEvalOutcome::Throw(StaticThrowValue::NamedError(
                "TypeError",
            )));
        }

        None
    }

    pub(in crate::backend::direct_wasm) fn eval_arguments_declaration_conflicts(
        &self,
        program: &Program,
    ) -> bool {
        if !eval_program_declares_var_arguments(program) {
            return false;
        }

        let Some(current_function) = self.current_user_function() else {
            return false;
        };

        !current_function.lexical_this
            || current_function
                .params
                .iter()
                .any(|parameter| parameter == "arguments")
    }

    pub(in crate::backend::direct_wasm) fn eval_program_declares_var_collision_with_global_lexical(
        &self,
        program: &Program,
    ) -> bool {
        if !self.state.speculation.execution_context.top_level_function || program.strict {
            return false;
        }

        collect_eval_var_names(program)
            .into_iter()
            .any(|name| self.backend.global_has_lexical_binding(&name))
    }

    pub(in crate::backend::direct_wasm) fn eval_program_declares_var_collision_with_active_lexical(
        &self,
        program: &Program,
    ) -> bool {
        if program.strict {
            return false;
        }

        collect_eval_var_names(program).into_iter().any(|name| {
            self.state
                .emission
                .lexical_scopes
                .active_eval_lexical_binding_counts
                .contains_key(&name)
        })
    }

    pub(in crate::backend::direct_wasm) fn eval_program_declares_non_definable_global_function(
        &self,
        program: &Program,
    ) -> bool {
        if !self.state.speculation.execution_context.top_level_function {
            return false;
        }

        program
            .functions
            .iter()
            .filter(|function| function.register_global)
            .any(|function| is_non_definable_global_name(&function.name))
    }

    pub(in crate::backend::direct_wasm) fn parse_eval_program_in_current_function_context(
        &self,
        source: &str,
    ) -> Option<Program> {
        let current_function_name = self.current_function_name()?;
        if self
            .resolve_home_object_name_for_function(current_function_name)
            .is_some()
            && source.contains("super")
        {
            if let Some(program) = self.parse_eval_program_in_method_context(source) {
                return Some(program);
            }
        }

        self.parse_eval_program_in_ordinary_function_context(source)
    }

    pub(in crate::backend::direct_wasm) fn parse_eval_program_in_ordinary_function_context(
        &self,
        source: &str,
    ) -> Option<Program> {
        let wrapper_name = "__ayy_eval_new_target_context__";
        let wrapped_source = format!("function {wrapper_name}() {{\n{source}\n}}");
        let mut wrapped_program = frontend::parse_script_goal(&wrapped_source).ok()?;
        let wrapper = wrapped_program
            .functions
            .iter()
            .find(|function| function.name == wrapper_name)
            .cloned()?;
        wrapped_program
            .functions
            .retain(|function| function.name != wrapper_name);

        Some(Program {
            strict: wrapper.strict,
            functions: wrapped_program.functions,
            statements: wrapper.body,
        })
    }

    pub(in crate::backend::direct_wasm) fn parse_eval_program_in_method_context(
        &self,
        source: &str,
    ) -> Option<Program> {
        let wrapper_property = "__ayy_eval_wrapper__";
        let wrapped_source = format!("({{{wrapper_property}() {{\n{source}\n}}}});");
        let mut wrapped_program = frontend::parse_script_goal(&wrapped_source).ok()?;
        let wrapper_name = wrapped_program.statements.iter().find_map(|statement| {
            let Statement::Expression(Expression::Object(entries)) = statement else {
                return None;
            };
            entries.iter().find_map(|entry| match entry {
                crate::ir::hir::ObjectEntry::Data { key, value }
                    if matches!(key, Expression::String(name) if name == wrapper_property) =>
                {
                    let Expression::Identifier(name) = value else {
                        return None;
                    };
                    Some(name.clone())
                }
                _ => None,
            })
        })?;
        let wrapper = wrapped_program
            .functions
            .iter()
            .find(|function| function.name == wrapper_name)
            .cloned()?;
        wrapped_program
            .functions
            .retain(|function| function.name != wrapper_name);

        Some(Program {
            strict: wrapper.strict,
            functions: wrapped_program.functions,
            statements: wrapper.body,
        })
    }
}
