use std::{fs, path::Path};

use anyhow::{Context, Result};

use crate::{CompileOptions, ir::hir::Program};

pub fn compile_if_supported(program: &Program, options: &CompileOptions) -> Result<bool> {
    let Some(wasm_bytes) = emit_wasm(program)? else {
        return Ok(false);
    };
    write_output(&options.output, &wasm_bytes)?;
    Ok(true)
}

pub fn emit_wasm(program: &Program) -> Result<Option<Vec<u8>>> {
    super::direct_wasm::try_emit_wasm(program)
}

pub fn emit_wasm_with_reason(program: &Program) -> std::result::Result<Vec<u8>, &'static str> {
    super::direct_wasm::emit_wasm_with_reason(program)
}

fn write_output(path: &Path, contents: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create output directory `{}`", parent.display()))?;
    }

    fs::write(path, contents)
        .with_context(|| format!("failed to write output file `{}`", path.display()))?;

    Ok(())
}
