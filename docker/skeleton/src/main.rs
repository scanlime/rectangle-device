use cargo_lock::Lockfile;
use std::collections::HashSet;

fn main() {
    let lockfile = Lockfile::load("Cargo.lock").unwrap();
    let mut memo = HashSet::new();

    for package in &lockfile.packages {
        if memo.insert(package.name.clone()) {
            println!("{} = \"{}\"", package.name, package.version);
        }
    }
}
