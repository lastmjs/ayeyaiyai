pub fn compile_if_supported(program: &Program, options: &CompileOptions) -> Result<bool> {
    let Some(wasm_bytes) = emit_wasm(program)? else {
        return Ok(false);
    };
    write_output(&options.output, &wasm_bytes)?;
    Ok(true)
}

pub fn emit_wasm(program: &Program) -> Result<Option<Vec<u8>>> {
    let mut compiler = DirectWasmCompiler::default();
    match compiler.compile(program) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(Unsupported(_)) => Ok(None),
    }
}

pub fn emit_wasm_with_reason(program: &Program) -> std::result::Result<Vec<u8>, &'static str> {
    let mut compiler = DirectWasmCompiler::default();
    compiler
        .compile(program)
        .map_err(|Unsupported(message)| message)
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

