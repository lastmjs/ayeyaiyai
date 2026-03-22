use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct CompileOptions {
    pub output: PathBuf,
    pub target: String,
}
