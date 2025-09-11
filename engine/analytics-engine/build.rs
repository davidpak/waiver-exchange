fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Skip protobuf compilation for now - we'll use manual definitions
    // TODO: Install protoc and enable protobuf compilation
    
    // Recompile if proto files change
    let proto_files = &["proto/analytics.proto"];
    for proto_file in proto_files {
        println!("cargo:rerun-if-changed={}", proto_file);
    }

    Ok(())
}
