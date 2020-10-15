use build_deps::rerun_if_changed_paths;
use std::{path::Path, process::Command};

fn main() {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let files_to_copy = ["package.json", "webpack.config.js"];

    for file in &files_to_copy {
        let dest = Path::new(&out_dir).join(file);
        let src = Path::new(file).canonicalize().unwrap();
        std::fs::copy(src.to_str().unwrap(), dest.to_str().unwrap()).unwrap();
        rerun_if_changed_paths(src.to_str().unwrap()).unwrap();
    }
    rerun_if_changed_paths("src/**").unwrap();

    Command::new("yarn")
        .current_dir(&out_dir)
        .arg("install")
        .status()
        .unwrap();

    Command::new("yarn")
        .current_dir(&out_dir)
        .env(
            "PATH",
            format!(
                "{}:{}",
                std::env::var("PATH").unwrap(),
                Path::new(&out_dir)
                    .join("node_modules/.bin")
                    .to_str()
                    .unwrap()
            ),
        )
        .arg("build")
        .status()
        .unwrap();
}
