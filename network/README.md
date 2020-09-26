rectangle-device-sandbox
========================

Light-weight isolated containers for streaming video transcoding pipelines. Builds on the shoulders of giants, ffmpeg and podman.

This is an attempt to build a platform for reproducible transcodes, using OSI/Docker images to reference specific builds of ffmpeg. The transcode pipelines communicate with the outside world through unix domain sockets that bind-mount into the container's filesystem.

