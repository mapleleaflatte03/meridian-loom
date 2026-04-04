use std::path::{Path, PathBuf};
use std::env;

fn default_kernel_path() -> PathBuf {
    if let Ok(path) = env::var("MERIDIAN_KERNEL_PATH") {
        return PathBuf::from(path);
    }

    let mut workspace_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    workspace_dir.pop();
    workspace_dir.pop();
    workspace_dir.join("kernel") // Is there a kernel dir in repo? Let's check
}

fn main() {
    println!("{:?}", default_kernel_path());
}
