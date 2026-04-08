use std::collections::HashSet;

use anyhow::{Context, Result, bail, ensure};
use swc_common::source_map::SmallPos;
use swc_ecma_ast::{UnaryOp as SwcUnaryOp, *};

mod strict_mode;
mod syntax;

pub(crate) use strict_mode::{
    function_has_use_strict_directive, script_has_use_strict_directive,
    validate_strict_mode_early_errors_in_module_items,
    validate_strict_mode_early_errors_in_statements,
};
pub(crate) use syntax::{
    collect_module_declared_names, collect_pattern_binding_names, collect_var_decl_bound_names,
    ensure_module_lexical_names_are_unique, validate_class_syntax, validate_declaration_syntax,
    validate_expression_syntax, validate_function_syntax, validate_statement_syntax,
};

fn source_slice_for_span<'a>(
    file: &'a swc_common::SourceFile,
    span: swc_common::Span,
) -> Result<&'a str> {
    let source: &str = file.src.as_ref();
    let start = span.lo.to_usize() - file.start_pos.to_usize();
    let end = span.hi.to_usize() - file.start_pos.to_usize();
    source
        .get(start..end)
        .context("expression span fell outside the source file")
}

pub(super) fn validate_import_attributes(attributes: Option<&ObjectLit>) -> Result<()> {
    let Some(attributes) = attributes else {
        return Ok(());
    };
    let import_with = attributes
        .as_import_with()
        .context("unsupported import attributes syntax")?;
    let mut keys = HashSet::new();
    for item in import_with.values {
        let key = item.key.sym.to_string();
        ensure!(
            keys.insert(key.clone()),
            "duplicate import attribute key `{key}`"
        );
    }
    Ok(())
}
