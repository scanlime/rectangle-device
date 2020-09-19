use std::process::Command;

fn main() {
    Command::new("yarn").arg("install").current_dir("player").status().unwrap();
    Command::new("yarn").arg("build").current_dir("player").status().unwrap();

    println!("cargo:rerun-if-changed=player/package.json");
    println!("cargo:rerun-if-changed=player/webpack.config.js");
    println!("cargo:rerun-if-changed=player/src/index.js");
}
