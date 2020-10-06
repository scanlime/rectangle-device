rectangle device
================

This is an experimental live + vod video server that integrates with IPFS for long-term distributed storage.

This is a proof of concept for streaming video ingest in a format that could be quickly shared via IPFS while the stream is still ongoing.

Build dependencies:
- rust -- get it [from rustup](https://rustup.rs/) if you like. the project builds with `cargo`.
- yarn -- yes, yarn, not npm. npm is bad at reproducible builds. the rust build system invokes yarn to build the javascript, so [install it](https://yarnpkg.com/).

Runtime dependencies:
- linux -- this project is basically linux-only due to the way it interacts
  with transcode containers. if you use another OS, run this inside docker or your favorite virtual machine.
- openssl -- just used temporarily for generating keys, as a bug workaround
- podman -- [get the thing](https://podman.io/getting-started/installation). this is a lightweight container manager and runtime which does not require any additional privileges to run. this is used to manage sandboxed reproducible transcodes, with hashed ffmpeg images.

