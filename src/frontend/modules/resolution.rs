use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};

pub(crate) fn normalize_module_path(path: &Path) -> Result<PathBuf> {
    path.canonicalize()
        .with_context(|| format!("failed to resolve module path `{}`", path.display()))
}

pub(crate) fn resolve_module_specifier(module_path: &Path, source: &str) -> Result<PathBuf> {
    ensure!(
        source.starts_with("./") || source.starts_with("../") || source.starts_with('/'),
        "unsupported module specifier `{source}`"
    );
    let candidate = if source.starts_with('/') {
        PathBuf::from(source)
    } else {
        module_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(source)
    };
    normalize_module_path(&candidate)
}
