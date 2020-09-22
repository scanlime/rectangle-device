// This code may not be used for any purpose. Be gay, do crime.

use std::error::Error;
use async_process::{Stdio, Child};
use crate::runtime;
use crate::socket::SocketPool;
use crate::types::ImageDigest;

pub fn default_image() -> ImageDigest {
    // Video transcoder image to use. This should be replaced with a locally preferred image
    // as well as a list of acceptable images that we'll be happy to run if we are trying to
    // reproduce a particular video block which requests it.
    // Also see: https://ffmpeg.org/security.html
    // Seems we want a combination of approaches, using a very restrictive sandbox plus
    // whitelisting versions of ffmpeg. As a result, videos that were uploaded using older
    // versions of ffmpeg may not find servers willing to re-run those transcodes if needed.

    ImageDigest::parse(
        "docker.io/jrottenberg/ffmpeg:4.3.1-scratch38",
        "68126e39534eff79a8a4a4b7b546a11b8165a1ee8f1af93166d3071b170280a1"
    ).unwrap()
}

#[derive(Clone, Debug)]
pub struct TranscodeConfig {
    pub image: ImageDigest,
    pub args: Vec<String>,
    pub allow_networking: bool,
    pub segment_time: f32,
}

pub async fn start(tc: TranscodeConfig, pool: &SocketPool) -> Result<Child, Box<dyn Error>> {

    if !runtime::image_exists(&tc.image).await? {
        runtime::pull(&tc.image).await?;
    }

    // Be specific about sandbox options
    let mut command = runtime::command();
    command
        .arg("run")
        .arg("--rm")
        .arg("--attach=stdout")
        .arg("--attach=stderr")
        .arg("--env-host=false")
        .arg("--read-only")
        .arg("--restart=no")
        .arg("--detach=false")
        .arg("--privileged=false");

    if tc.allow_networking {
        command
            .arg("--net=slirp4netns")
            .arg("--dns-search=.");
    } else {
        command.arg("--net=none");
    }

    command
        .args(&pool.mount_args)
        .arg(tc.image.digest.as_str())
        .args(tc.args);

    // Additional args for ffmpeg

    command
        .arg("-nostats")
        .arg("-nostdin")
        .arg("-loglevel").arg("error")
        .arg("-f").arg("stream_segment")
        .arg("-segment_format").arg("mpegts")
        .arg("-segment_wrap").arg("1")
        .arg("-segment_time").arg(tc.segment_time.to_string())
        .arg("unix:///out/%d.ts");

    log::info!("starting, {:?}", command);

    Ok(command
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?)
}
