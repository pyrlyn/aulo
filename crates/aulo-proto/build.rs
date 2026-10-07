//! Compiles `proto/aulo/v1/*.proto` into tonic server and client code.
//!
//! `protoc` is taken from `PROTOC` or `PATH`: a vendored binary would pin one
//! version for every platform, while buf already requires a local toolchain.

use std::error::Error;
use std::path::{Path, PathBuf};
use std::{env, fs};

const PROTO_ROOT: &str = "../../proto";
const PROTO_PACKAGE_DIR: &str = "aulo/v1";

fn main() -> Result<(), Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let root = manifest_dir.join(PROTO_ROOT);
    let files = proto_files(&root.join(PROTO_PACKAGE_DIR))?;

    // Directory-level watch so a new .proto file also triggers a rebuild.
    println!("cargo:rerun-if-changed={}", root.display());
    println!("cargo:rerun-if-env-changed=PROTOC");

    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&files, &[root])?;
    Ok(())
}

// Sorted so the generated output does not depend on directory iteration order.
fn proto_files(dir: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().is_some_and(|ext| ext == "proto") {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}
