use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use prost::Message;
use std::env;
use std::path::Path;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tonic::transport::Server;
use tonic_reflection::server::Builder;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "0.0.0.0:50051".parse()?;
    let proto_path = env::var("PROTO_PATH")
        .unwrap_or_else(|_| "/etc/protos/interface-definitions/proto".to_string());

    println!("🚀 Starting Reflection Provider");

    // Initial Compilation at Runtime
    let encoded_set = compile_protos(&proto_path)?;

    // Health Check Service
    let (health_reporter, health_service) = tonic_health::server::health_reporter();
    // health_reporter.set_serving::<()>().await;
    health_reporter
        .set_service_status("", tonic_health::ServingStatus::Serving)
        .await;
    // Mark all discovered services as SERVING in the health reporter
    // This makes 'grpcurl ... Health/Check' work for specific service names
    let descriptor_set = prost_types::FileDescriptorSet::decode(&encoded_set[..])?;
    for file in &descriptor_set.file {
        for service in &file.service {
            let full_name = format!(
                "{}.{}",
                file.package.as_deref().unwrap_or(""),
                service.name.as_deref().unwrap_or("")
            );
            health_reporter
                .set_service_status(full_name, tonic_health::ServingStatus::Serving)
                .await;
        }
    }

    // Setup Watcher for git-sync updates
    let (tx, mut rx) = mpsc::channel(1);
    let mut watcher = RecommendedWatcher::new(
        move |res: notify::Result<Event>| {
            if let Ok(event) = res {
                // Only trigger on actual data modifications or link swaps
                if event.kind.is_modify() || event.kind.is_create() {
                    let _ = tx.blocking_send(());
                }
            }
        },
        Config::default(),
    )?;

    // Watch the parent directory of the proto root to catch symlink swaps
    if let Some(parent) = Path::new(&proto_path).parent() {
        watcher.watch(parent, RecursiveMode::Recursive)?;
    } else {
        watcher.watch(Path::new(&proto_path), RecursiveMode::Recursive)?;
    }

    println!("📂 Monitoring Protos at: {proto_path}");

    let start_time = Instant::now();

    tokio::spawn(async move {
        while rx.recv().await.is_some() {
            // Ignore events that happen within the first 30 seconds of startup
            // to avoid catching the event that might have just triggered a restart.
            if start_time.elapsed() > Duration::from_secs(30) {
                println!("🔄 git-sync updated the protos! Exiting to trigger reload...");
                std::process::exit(0);
            }
        }
    });

    // Setup Reflection Service
    let reflection_service = Builder::configure()
        .register_encoded_file_descriptor_set(&encoded_set)
        .register_encoded_file_descriptor_set(tonic_health::pb::FILE_DESCRIPTOR_SET)
        .build_v1()?;

    println!("✅ gRPC Server listening on {addr}");

    Server::builder()
        .add_service(health_service)
        .add_service(reflection_service)
        .serve(addr)
        .await?;

    Ok(())
}

fn compile_protos(path: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use walkdir::WalkDir;

    // Use WalkDir to find all .proto files recursively
    let proto_files: Vec<_> = WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "proto"))
        .map(|e| e.into_path())
        .collect();

    if proto_files.is_empty() {
        return Err(format!("No .proto files found in {path}").into());
    }

    println!("🔍 Found {} proto files", proto_files.len());

    // Resolve the parent directory so that imports starting with "proto/..." work
    let proto_root = Path::new(path)
        .parent()
        .ok_or("Unable to determine parent of PROTO_PATH")?;

    // Protox needs the base 'includes' path to resolve imports correctly.
    // We pass the root proto directory as the include path.
    let descriptor_set = protox::compile(proto_files, [proto_root])?;

    let mut buf = Vec::with_capacity(descriptor_set.encoded_len());
    descriptor_set.encode(&mut buf)?;

    println!("📦 Compiled {} descriptors", descriptor_set.file.len());
    Ok(buf)
}
