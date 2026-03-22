pub mod backend;
mod compile_options;
pub mod frontend;
pub mod ir;

use std::path::Path;

use anyhow::{bail, Result};

pub use backend::{emit_wasm, emit_wasm_with_reason};
pub use compile_options::CompileOptions;

pub fn compile_file(path: &Path, options: &CompileOptions) -> Result<()> {
    let program = frontend::bundle_script_entry(path)?;
    ir::aot::validate_refined_aot(&program)?;
    let program = ir::aot::lower_static_function_constructors(program)?;
    if backend::compile_if_supported(&program, options)? {
        return Ok(());
    }

    bail!("program uses JavaScript features that are not yet supported by the direct wasm backend")
}

pub fn compile_file_with_goal(path: &Path, options: &CompileOptions, module: bool) -> Result<()> {
    let program = if module {
        frontend::bundle_module_entry(path)?
    } else {
        frontend::bundle_script_entry(path)?
    };
    ir::aot::validate_refined_aot(&program)?;
    let program = ir::aot::lower_static_function_constructors(program)?;
    if backend::compile_if_supported(&program, options)? {
        return Ok(());
    }
    bail!("program uses JavaScript features that are not yet supported by the direct wasm backend")
}

pub fn compile_source(source: &str, options: &CompileOptions) -> Result<()> {
    compile_source_with_goal(source, options, false)
}

pub fn compile_source_with_goal(
    source: &str,
    options: &CompileOptions,
    module: bool,
) -> Result<()> {
    let program = if module {
        frontend::parse_module_goal(source)?
    } else {
        frontend::parse(source)?
    };
    ir::aot::validate_refined_aot(&program)?;
    let program = ir::aot::lower_static_function_constructors(program)?;
    if backend::compile_if_supported(&program, options)? {
        return Ok(());
    }
    bail!("program uses JavaScript features that are not yet supported by the direct wasm backend")
}

pub fn compile_source_with_reason(source: &str) -> std::result::Result<(), String> {
    let program = frontend::parse(source).map_err(|_| "source failed to parse".to_string())?;
    ir::aot::validate_refined_aot(&program).map_err(|_| "aot validation failed".to_string())?;
    let program = ir::aot::lower_static_function_constructors(program)
        .map_err(|_| "aot lowering failed".to_string())?;
    match backend::emit_wasm_with_reason(&program) {
        Ok(_) => Ok(()),
        Err(message) => Err(message.to_string()),
    }
}

#[cfg(test)]
mod lib_tests;
