// build.rs - Compiles JITO protobuf definitions into Rust code
//
// This runs at build time to generate Rust types from .proto files

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Compile JITO protobuf definitions
    tonic_build::configure()
        .build_server(false)  // We're a client, not a server
        .compile(
            &[
                "proto/searcher.proto",  // SearcherService with SendBundle RPC
                "proto/bundle.proto",    // Bundle, BundleResult types
                "proto/packet.proto",    // Packet type (transaction wrapper)
                "proto/shared.proto",    // Header type (timestamp)
            ],
            &["proto"],  // Include directory
        )?;

    println!("cargo:rerun-if-changed=proto/");

    Ok(())
}
