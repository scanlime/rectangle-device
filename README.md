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

----

notes & junk
............

Big questions to answer:
- how does this interact with transcoding? seems like block transcoding is basically okay but we probably need to include a backtrack by one segment in the stream to be sure we get enough data to decode a complete frame. tempting to dive into h264 here but it would be great to avoid getting too low-level to make it easier to support new codecs.
- when can this server decide to discard blocks? combination of block usage info + which (trusted?) peers have requested it + timestamps
- how do we represent instructions for transforming discardable blocks? using containers and podman for this seems to be going well so far. how do transformations from URIs vs block-oriented sources work? want to keep the transcoding/splitting ffmpeg containers and the downloading ffmpeg/youtube-dl containers totally separate.
- get more specific about how this supports dash, additional codecs, additional formats that we present over http/whatever for federation/compatibility

Just code:
- tools to debug bandwidth and timing in libp2p and bitswap
- js frontend updates live stream using pubsub
- fix player bugs with mpegts-level blocks in playlists
- use hashes in m3u playlist to remove a layer of indirection
- add some api parts and try multiple streams, starting/stopping them dynamically

