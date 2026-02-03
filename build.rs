use std::env;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let proto_root = Path::new("interface-definitions/proto");

    // If the submodule hasn't been initialized yet, we should bail gracefully
    // instead of letting walkdir return an empty vec.
    if !proto_root.exists() {
        println!(
            "cargo:warning=Proto root not found at {}. Skipping compilation.",
            proto_root.display()
        );
        return Ok(());
    }

    let protos: Vec<PathBuf> = WalkDir::new(proto_root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "proto"))
        .map(|e| e.into_path())
        .collect();

    // Tell Cargo to rerun this script if any proto changes
    for proto in &protos {
        println!("cargo:rerun-if-changed={}", proto.display());
    }

    tonic_prost_build::configure()
        .file_descriptor_set_path(out_dir.join("service_descriptor.bin"))
        .build_server(false) // This is just a catalog, we don't need service traits
        .build_client(false)
        .compile_protos(&protos, &[proto_root.to_path_buf()])?;

    Ok(())
}
