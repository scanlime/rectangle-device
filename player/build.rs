use std::process::Command;
use build_deps::rerun_if_changed_paths;
use std::path::Path;
use std::env;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let node_modules = Path::new(&out_dir).join("node_modules");
    let yarn_args = vec![ "--modules-folder", node_modules.to_str().unwrap() ];

    Command::new("yarn").args(&yarn_args).arg("install").status().unwrap();
    Command::new("yarn").args(&yarn_args).arg("build").status().unwrap();

    rerun_if_changed_paths( "package.json" ).unwrap();
    rerun_if_changed_paths( "webpack.config.js" ).unwrap();
    rerun_if_changed_paths( "src/**" ).unwrap();
}
