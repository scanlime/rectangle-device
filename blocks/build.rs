fn main() {
    prost_build::compile_protos(&["src/unixfs/unixfs.proto"], &["src/unixfs"]).unwrap();
}
