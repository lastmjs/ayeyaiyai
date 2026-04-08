use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn parse_static_eval_program_in_context(
        &self,
        source: &str,
        current_function_name: Option<&str>,
    ) -> Option<Program> {
        if let Some(current_function_name) = current_function_name {
            if self
                .resolve_home_object_name_for_function_static(current_function_name)
                .is_some()
                && source.contains("super")
                && let Some(program) = self.parse_eval_program_in_method_context_static(source)
            {
                return Some(program);
            }
            if let Some(program) =
                self.parse_eval_program_in_ordinary_function_context_static(source)
            {
                return Some(program);
            }
        }
        frontend::parse(source).ok()
    }

    pub(in crate::backend::direct_wasm) fn resolve_home_object_name_for_function_static(
        &self,
        function_name: &str,
    ) -> Option<String> {
        if let Some(home_object_name) = self.user_function_home_object_binding(function_name) {
            return Some(home_object_name);
        }
        self.find_global_home_object_binding_name(function_name)
    }

    pub(in crate::backend::direct_wasm) fn parse_eval_program_in_ordinary_function_context_static(
        &self,
        source: &str,
    ) -> Option<Program> {
        let wrapper_name = "__ayy_eval_new_target_context__";
        let wrapped_source = format!("function {wrapper_name}() {{\n{source}\n}}");
        let mut wrapped_program = frontend::parse(&wrapped_source).ok()?;
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

    pub(in crate::backend::direct_wasm) fn parse_eval_program_in_method_context_static(
        &self,
        source: &str,
    ) -> Option<Program> {
        let wrapper_property = "__ayy_eval_wrapper__";
        let wrapped_source = format!("({{{wrapper_property}() {{\n{source}\n}}}});");
        let mut wrapped_program = frontend::parse(&wrapped_source).ok()?;
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
