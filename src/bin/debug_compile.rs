use std::env;
use std::fs;

use ayeyaiyai::{CompileOptions, compile_file, compile_source_with_reason, frontend};

fn main() {
    let path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: debug_compile <path>");
        std::process::exit(1);
    });

    let source = fs::read_to_string(&path).unwrap_or_else(|error| {
        eprintln!("failed to read `{}`: {}", path, error);
        std::process::exit(1);
    });

    if env::var_os("AYY_DEBUG_HIR").is_some() {
        let program_result = if env::var_os("AYY_DEBUG_BUNDLED_HIR").is_some() {
            if env::var_os("AYY_DEBUG_MODULE_BUNDLED_HIR").is_some() {
                frontend::bundle_module_entry(std::path::Path::new(&path))
            } else {
                frontend::bundle_script_entry(std::path::Path::new(&path))
            }
        } else {
            frontend::parse(&source)
        };
        match program_result {
            Ok(program) => {
                println!("{program:#?}");
            }
            Err(error) => {
                eprintln!("parse error: {error:#}");
                std::process::exit(1);
            }
        }
    }

    if env::var_os("AYY_DEBUG_FILE_COMPILE").is_some() {
        let output = env::temp_dir().join("ayy-debug-file-compile.wasm");
        let options = CompileOptions {
            output: output.clone(),
            target: "wasm32-wasip2".to_string(),
        };
        let result = compile_file(std::path::Path::new(&path), &options);
        let _ = fs::remove_file(output);
        match result {
            Ok(()) => println!("ok"),
            Err(error) => println!("unsupported: {error:#}"),
        }
        return;
    }

    match compile_source_with_reason(&source) {
        Ok(_) => println!("ok"),
        Err(message) => println!("unsupported: {message}"),
    }
}
