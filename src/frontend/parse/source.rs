use std::{borrow::Cow, fs, path::Path};

use anyhow::{Context, Result, bail};
use swc_common::{FileName, SourceMap, sync::Lrc};
use swc_ecma_ast::{Module, Program as SwcProgram};
use swc_ecma_parser::{EsSyntax, Parser, StringInput, Syntax, lexer::Lexer};

use super::{await_rewrite::rewrite_script_await_identifiers, validation::*};

pub(super) fn parse_program_source(source: &str) -> Result<SwcProgram> {
    let file = source_file(FileName::Custom("input.js".into()), source);

    parse_script(&file).or_else(|script_error| {
        parse_module(&file).map_err(|module_error| {
            anyhow::anyhow!(
                "failed to parse JavaScript source as script: {script_error:#}\nfailed to parse JavaScript source as module: {module_error:#}"
            )
        })
    })
}

pub(super) fn parse_script_program_source(source: &str) -> Result<SwcProgram> {
    let file = source_file(FileName::Custom("input.js".into()), source);
    parse_script(&file)
}

pub(super) fn parse_module_program_with_path(path: &Path, source: &str) -> Result<SwcProgram> {
    let file = source_file(FileName::Real(path.to_path_buf()).into(), source);
    parse_module(&file)
}

pub(super) fn validate_script_source(source: &str) -> Result<()> {
    let file = source_file(FileName::Custom("eval.js".into()), source);
    parse_script(&file).map(|_| ())
}

pub(crate) fn parse_module_file(path: &Path) -> Result<(Module, String)> {
    let source =
        fs::read_to_string(path).with_context(|| format!("failed to read `{}`", path.display()))?;
    let file = source_file(FileName::Real(path.to_path_buf()).into(), &source);
    let SwcProgram::Module(module) = parse_module(&file)? else {
        unreachable!("parse_module must return a module");
    };
    Ok((module, source))
}

pub(crate) fn parse_script_file(path: &Path) -> Result<(swc_ecma_ast::Script, String)> {
    let source =
        fs::read_to_string(path).with_context(|| format!("failed to read `{}`", path.display()))?;

    parse_script_file_once(path, &source)
        .map(|script| (script, source.clone()))
        .or_else(|parse_error| {
            let Some(rewritten) = rewrite_script_await_identifiers(&source) else {
                return Err(parse_error);
            };
            parse_script_file_once(path, &rewritten)
                .map(|script| (script, rewritten))
                .map_err(|rewrite_error| {
                    anyhow::anyhow!(
                        "{parse_error:#}\nfailed again after rewriting script-goal `await` identifiers: {rewrite_error:#}"
                    )
                })
        })
}

fn source_file(file_name: FileName, source: &str) -> Lrc<swc_common::SourceFile> {
    let normalized = normalize_leading_hashbang_comment(source);
    let source_map: Lrc<SourceMap> = Default::default();
    source_map.new_source_file(file_name.into(), normalized.into_owned())
}

fn normalize_leading_hashbang_comment(source: &str) -> Cow<'_, str> {
    if let Some(rest) = source.strip_prefix("#!") {
        return Cow::Owned(format!("//{rest}"));
    }

    if let Some(rest) = source.strip_prefix("\u{FEFF}#!") {
        return Cow::Owned(format!("\u{FEFF}//{rest}"));
    }

    Cow::Borrowed(source)
}

fn parse_script(file: &swc_common::SourceFile) -> Result<SwcProgram> {
    let lexer = Lexer::new(
        script_syntax(),
        Default::default(),
        StringInput::from(file),
        None,
    );
    let mut parser = Parser::new_from(lexer);
    let script = parser
        .parse_script()
        .map_err(|error| anyhow::anyhow!("{error:?}"))?;
    if let Some(error) = parser.take_errors().into_iter().next() {
        bail!("{error:?}");
    }
    validate_script_ast(&script, file)?;
    Ok(SwcProgram::Script(script))
}

fn parse_module(file: &swc_common::SourceFile) -> Result<SwcProgram> {
    let lexer = Lexer::new(
        script_syntax(),
        Default::default(),
        StringInput::from(file),
        None,
    );
    let mut parser = Parser::new_from(lexer);
    let module = parser
        .parse_module()
        .map_err(|error| anyhow::anyhow!("{error:?}"))?;
    if let Some(error) = parser.take_errors().into_iter().next() {
        bail!("{error:?}");
    }
    validate_module_ast(&module, file)?;
    Ok(SwcProgram::Module(module))
}

fn parse_script_file_once(path: &Path, source: &str) -> Result<swc_ecma_ast::Script> {
    let file = source_file(FileName::Real(path.to_path_buf()).into(), source);
    let SwcProgram::Script(script) = parse_script(&file)? else {
        unreachable!("parse_script must return a script");
    };
    Ok(script)
}

fn script_syntax() -> Syntax {
    Syntax::Es(EsSyntax {
        decorators: true,
        decorators_before_export: true,
        ..Default::default()
    })
}
