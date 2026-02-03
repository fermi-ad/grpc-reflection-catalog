use std::path::Path;
use std::time::Duration;
use tokio::time::sleep;
use tonic::transport::Server;
use walkdir::WalkDir;

async fn get_descriptor_set(proto_path: &Path) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let protos: Vec<_> = WalkDir::new(proto_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "proto"))
        .map(|e| e.into_path())
        .collect();

    let descriptor_set_path = std::env::temp_dir().join("descriptor.bin");

    // We use prost_build to create the descriptor set in memory
    // Note: This requires 'protoc' to be installed in the runtime image
    tonic_prost_build::configure()
        .file_descriptor_set_path(&descriptor_set_path)
        .build_server(false)
        .compile_protos(&protos, &[proto_path])?;

    let bytes = std::fs::read(descriptor_set_path)?;

    Ok(bytes)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::0]:50051".parse()?;

    // Check environment variable, fallback to K8s defined path
    let proto_path_env = std::env::var("PROTO_PATH")
        .unwrap_or_else(|_| "/etc/protos/interface-definitions/proto".to_string());

    let proto_dir = Path::new(&proto_path_env);

    // Only loop/wait if we are in the K8s environment
    if proto_path_env.starts_with("/etc/protos") {
        (1..=20).into_iter().any(|attempt| {
            if proto_dir.exists() {
                true
            } else {
                println!(
                    "Waiting for git-sync at {} (Attempt {}/20)...",
                    proto_dir.display(),
                    attempt
                );
                tokio::time::sleep(Duration::from_secs(5)).await;
                false
            }
        });

        // This accounts for the case where all 20 attempts fail.
        if !proto_dir.exists() {
            panic!("Timed out waiting for git-sync!");
        }
    } else if !proto_dir.exists() {
        // Exit early if local path is wrong to avoid confusion
        panic!("Local PROTO_PATH {} does not exist!", proto_dir.display());
    }

    let descriptor_set = get_descriptor_set(proto_dir).await?;

    let reflection_service = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(&descriptor_set)
        .build_result()?;

    println!("Reflection Catalog listening on {}", addr);

    Server::builder()
        .add_service(reflection_service)
        .serve(addr)
        .await?;

    Ok(())
}
