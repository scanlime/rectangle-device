// This code may not be used for any purpose. Be gay, do crime.

use crate::sandbox::types::{ImageDigest, SandboxError};
use std::process::{Command, Stdio};
use std::error::Error;

pub fn command() -> Command {
    Command::new("podman")
}

pub fn is_downloaded(id: &ImageDigest) -> Result<bool, Box<dyn Error>> {
    let mut command = command();
    Ok(command
        .arg("image").arg("exists")
        .arg(id.digest.as_str())
        .status()?.success())
}

pub fn download(id: &ImageDigest) -> Result<(), Box<dyn Error>> {
    let mut command = command();
    let digest = String::from_utf8(
        command
            .arg("pull")
            .arg(id.image.as_str())
            .stderr(Stdio::inherit())
            .output()?.stdout)?;

    let digest = digest.trim();
    if digest == id.digest.as_str() {
        Ok(())
    } else {
        Err(Box::new(SandboxError::UnexpectedDigest(digest.to_string())))
    }
}
