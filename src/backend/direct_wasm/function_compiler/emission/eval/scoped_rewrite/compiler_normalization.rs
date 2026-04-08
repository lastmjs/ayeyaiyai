use super::*;

#[path = "compiler_normalization/binding_names.rs"]
mod binding_names;
#[path = "compiler_normalization/declaration_collection.rs"]
mod declaration_collection;
#[path = "compiler_normalization/expression_rewrite.rs"]
mod expression_rewrite;
#[path = "compiler_normalization/statement_rewrite.rs"]
mod statement_rewrite;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn normalize_eval_scoped_bindings_to_source_names(
        &self,
        program: &mut Program,
    ) {
        Self::normalize_eval_scoped_bindings_to_source_names_impl(program);
    }

    pub(in crate::backend::direct_wasm) fn normalize_eval_scoped_bindings_to_source_names_impl(
        program: &mut Program,
    ) {
        let eval_local_function_bindings = Self::collect_eval_local_function_bindings(program);
        let declared_bindings = Self::collect_eval_scoped_declared_bindings(program);

        for statement in &mut program.statements {
            Self::rewrite_eval_scoped_captures_in_statement(
                statement,
                &declared_bindings,
                &eval_local_function_bindings,
            );
        }
        for function in &mut program.functions {
            for parameter in &mut function.params {
                Self::rewrite_eval_scoped_binding_name(
                    &mut parameter.name,
                    &declared_bindings,
                    &eval_local_function_bindings,
                );
                if let Some(default) = &mut parameter.default {
                    Self::rewrite_eval_scoped_captures_in_expression(
                        default,
                        &declared_bindings,
                        &eval_local_function_bindings,
                    );
                }
            }
            if let Some(binding) = &mut function.top_level_binding {
                Self::rewrite_eval_scoped_binding_name(
                    binding,
                    &declared_bindings,
                    &eval_local_function_bindings,
                );
            }
            if let Some(binding) = &mut function.self_binding {
                Self::rewrite_eval_scoped_binding_name(
                    binding,
                    &declared_bindings,
                    &eval_local_function_bindings,
                );
            }
            for statement in &mut function.body {
                Self::rewrite_eval_scoped_captures_in_statement(
                    statement,
                    &declared_bindings,
                    &eval_local_function_bindings,
                );
            }
        }
    }
}
