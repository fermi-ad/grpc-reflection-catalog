use prost::Message;
use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let proto_path = env::var("PROTO_PATH")
        .unwrap_or_else(|_| "/etc/protos/interface-definitions/proto".to_string());

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let descriptor_path = out_dir.join("proto_descriptor.bin");

    // Tell Cargo to re-run if any file in the proto directory changes
    println!("cargo:rerun-if-changed={}", proto_path);

    // Collect all .proto files
    let mut protos = Vec::new();
    if let Ok(entries) = fs::read_dir(&proto_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension() == Some("proto".as_ref()) {
                protos.push(path);
            }
        }
    }

    if protos.is_empty() {
        // If no protos yet (git-sync still working), write an empty set so include_bytes! doesn't fail
        fs::write(&descriptor_path, vec![]).unwrap();
        return;
    }

    // Compile into FileDescriptorSet
    let descriptor_set = protox::compile(protos, &[proto_path]).expect("Failed to compile protos");

    // Encode the struct into actual bytes
    let mut buf = Vec::new();
    descriptor_set
        .encode(&mut buf)
        .expect("Failed to encode descriptor set");

    fs::write(descriptor_path, buf).expect("Failed to write descriptor set binary");
}
