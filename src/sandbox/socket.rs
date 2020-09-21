// This code may not be used for any purpose. Be gay, do crime.

use crate::config;
use async_std::os::unix::net::UnixListener;
use std::error::Error;
use std::fs::Permissions;
use std::os::unix::fs::PermissionsExt;
use tempfile::TempDir;

pub struct SocketPool {
    pub mount_args: Vec<String>,
    dir: TempDir,
}

impl SocketPool {
    pub fn new() -> Result<SocketPool, Box<dyn Error>> {
        let dir = tempfile::Builder::new().prefix(config::TEMP_DIR_PREFIX).tempdir()?;
        std::fs::set_permissions(&dir, Permissions::from_mode(config::TEMP_DIR_MODE))?;
        Ok(SocketPool {
            mount_args: vec![],
            dir
        })
    }

    pub async fn bind(&mut self, path_in_container: &str) -> Result<UnixListener, Box<dyn Error>> {
        let arbitrary_id = self.mount_args.len().to_string();
        let path = self.dir.path().join(arbitrary_id);
        self.mount_args.push(format!("-v={}:{}", path.to_str().unwrap(), path_in_container));
        Ok(UnixListener::bind(path).await?)
    }
}
