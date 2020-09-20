use std::process::Command;
use build_deps::rerun_if_changed_paths;

fn main() {
    Command::new("yarn").arg("install").status().unwrap();
    Command::new("yarn").arg("build").status().unwrap();
    rerun_if_changed_paths( "package.json" ).unwrap();
    rerun_if_changed_paths( "webpack.config.js" ).unwrap();
    rerun_if_changed_paths( "src/**" ).unwrap();
}
