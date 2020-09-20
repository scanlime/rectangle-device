// This code may not be used for any purpose. Be gay, do crime.

use std::error::Error;
use std::process::Stdio;
use crate::config;
use crate::sandbox::runtime;
use crate::sandbox::types::ImageDigest;

pub fn run(args: Vec<String>) -> Result<(), Box<dyn Error>> {

    let image = ImageDigest::parse(config::FFMPEG_CONTAINER_NAME, config::FFMPEG_CONTAINER_HASH)?;

    if !runtime::is_downloaded(&image)? {
        runtime::download(&image)?;
    }

    let mut command = runtime::command();
    command
        .arg("run")
        .arg("-a").arg("stdout")
        .arg("-a").arg("stderr")
        .arg("--net=slirp4netns")
        .arg("--dns-search=.")
        .arg("--env-host=false")
        .arg("--read-only")
        .arg("--restart=no")
        .arg("--detach=false")
        .arg("--privileged=false")
        //.args(socket_ring.mount_args.clone())
        .arg(image.digest.as_str())
        .args(args)
        .arg("-nostats").arg("-nostdin")
        .arg("-loglevel").arg("error")
        .arg("-c").arg("copy")
        .arg("-f").arg("stream_segment")
        .arg("-segment_format").arg("mpegts")
        //.arg("-segment_wrap").arg(NUM_SOCKETS.to_string())
        .arg("-segment_time").arg(config::SEGMENT_MIN_SEC.to_string());
//        .arg(format!("unix://{}/%d.ts", SOCKET_MOUNT));

    log::info!("using command: {:?}", command);

    let mut _result = command
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn().unwrap();

    Ok(())
}
