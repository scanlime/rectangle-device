use std::process::Command;
use std::path::Path;

fn main() {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let files_to_copy = [
        "package.json",
    ];

    for file in &files_to_copy {
        let dest = Path::new(&out_dir).join(file);
        let src = Path::new(file).canonicalize().unwrap();
        std::fs::copy(src.to_str().unwrap(), dest.to_str().unwrap()).unwrap();
    }

    Command::new("yarn")
        .current_dir(&out_dir)
        .arg("install").status().unwrap();
}
