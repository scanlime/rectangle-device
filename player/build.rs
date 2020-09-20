use std::process::Command;

fn main() {
    Command::new("yarn").arg("install").status().unwrap();
    Command::new("yarn").arg("build").status().unwrap();

    println!("cargo:rerun-if-changed=package.json");
    println!("cargo:rerun-if-changed=webpack.config.js");
    println!("cargo:rerun-if-changed=src/index.js");
}
