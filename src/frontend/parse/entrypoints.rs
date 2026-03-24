use std::path::Path;

use anyhow::Result;

use crate::{frontend::lowering::Lowerer, ir::hir::Program};

use super::{
    await_rewrite::rewrite_script_await_identifiers,
    source::{
        parse_module_program_with_path, parse_program_source, parse_script_program_source,
        validate_script_source,
    },
};

pub fn parse(source: &str) -> Result<Program> {
    let mut lowered_source = source.to_string();
    let parsed = parse_program_source(source).or_else(|parse_error| {
        let Some(rewritten) = rewrite_script_await_identifiers(source) else {
            return Err(parse_error);
        };
        lowered_source = rewritten;
        parse_program_source(&lowered_source).map_err(|rewrite_error| {
            anyhow::anyhow!(
                "{parse_error:#}\nfailed again after rewriting script-goal `await` identifiers: {rewrite_error:#}"
            )
        })
    })?;

    Lowerer::with_source_text(lowered_source).lower_program(&parsed)
}

pub fn parse_script_goal(source: &str) -> Result<Program> {
    let mut lowered_source = source.to_string();
    let parsed = parse_script_program_source(source).or_else(|parse_error| {
        let Some(rewritten) = rewrite_script_await_identifiers(source) else {
            return Err(parse_error);
        };
        lowered_source = rewritten;
        parse_script_program_source(&lowered_source).map_err(|rewrite_error| {
            anyhow::anyhow!(
                "{parse_error:#}\nfailed again after rewriting script-goal `await` identifiers: {rewrite_error:#}"
            )
        })
    })?;

    Lowerer::with_source_text(lowered_source).lower_program(&parsed)
}

pub fn parse_module_goal(source: &str) -> Result<Program> {
    parse_module_goal_with_path(Path::new("input.js"), source)
}

#[allow(dead_code)]
pub fn validate_script_goal(source: &str) -> Result<()> {
    validate_script_source(source)
}

pub fn parse_module_goal_with_path(path: &Path, source: &str) -> Result<Program> {
    let parsed = parse_module_program_with_path(path, source)?;
    Lowerer::with_source_text(source.to_string()).lower_program(&parsed)
}
