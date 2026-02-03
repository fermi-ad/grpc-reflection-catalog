use notify::{Event, RecursiveMode, Watcher};
use prost_types::FileDescriptorSet;
use protox::Compiler;
use std::path::Path;
use std::time::Duration;
use tonic::transport::Server;
use tonic_reflection::server::Builder;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse()?;
    let proto_root = std::env::var("PROTO_PATH")
        .unwrap_or_else(|_| "/etc/protos/interface-definitions/proto".to_string());

    println!("Looking for protos in: {}", proto_root);

    while !Path::new(&proto_root).exists() {
        println!("Waiting for git-sync to populate {}...", proto_root);
        tokio::time::sleep(Duration::from_secs(2)).await;
    }

    let initial_fds = compile_protos(&proto_root)?;
    let (mut health_reporter, health_service) = tonic_health::server::health_reporter();

    health_reporter
        .set_serving_status("", tonic_health::ServingStatus::Serving)
        .await;

    let reflection_service = Builder::configure()
        .register_encoded_file_descriptor_set(initial_fds.encode_to_vec().as_slice())
        .build_v1()?;

    println!("gRPC Server listening on {}", addr);

    Server::builder()
        .add_service(health_service)
        .add_service(reflection_service)
        .serve(addr)
        .await?;

    Ok(())
}

fn compile_protos(root: &str) -> Result<FileDescriptorSet, Box<dyn std::error::Error>> {
    let mut compiler = Compiler::new([root])?;

    // Find all .proto files in the directory
    let files: Vec<_> = walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "proto"))
        .map(|e| e.path().to_path_buf())
        .collect();

    for file in &files {
        compiler.add_file(file)?;
    }

    Ok(compiler.file_descriptor_set())
}
