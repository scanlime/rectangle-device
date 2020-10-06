use async_std::os::unix::net::UnixListener;
use std::error::Error;
use std::fs::Permissions;
use std::path::PathBuf;
use std::os::unix::fs::PermissionsExt;
use tempfile::TempDir;

const TEMP_DIR_PREFIX: &'static str = "rect-socket.";
const TEMP_DIR_MODE: u32 = 0o700;

pub struct SocketPool {
    pub mount_args: Vec<String>,
    dir: TempDir,
}

pub struct Socket {
    pub socket_path: PathBuf,
    pub listener: UnixListener,
}

impl SocketPool {
    pub fn new() -> Result<SocketPool, Box<dyn Error>> {
        let dir = tempfile::Builder::new().prefix(TEMP_DIR_PREFIX).tempdir()?;
        std::fs::set_permissions(&dir, Permissions::from_mode(TEMP_DIR_MODE))?;
        Ok(SocketPool {
            mount_args: vec![],
            dir
        })
    }

    pub async fn bind(&mut self, path_in_container: &str) -> Result<Socket, Box<dyn Error>> {
        let arbitrary_id = self.mount_args.len().to_string();
        let socket_path = self.dir.path().join(arbitrary_id);
        let socket_path_str = socket_path.to_str().unwrap();
        self.mount_args.push(format!("-v={}:{}", socket_path_str, path_in_container));
        let listener = UnixListener::bind(&socket_path).await?;
        Ok(Socket{ listener, socket_path })
    }
}
