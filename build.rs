fn main() {
    let proto = "proto/ai_agents.proto";
    println!("cargo:rerun-if-changed={proto}");
    let protoc = protoc_bin_vendored::protoc_bin_path().expect("vendored protoc path");
    std::env::set_var("PROTOC", protoc);
    tonic_build::configure()
        .build_server(true)
        .compile_protos(&[proto], &["proto"])
        .expect("compile ai_agents.proto");
}
