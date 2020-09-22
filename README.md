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


notes & junk
------------

- test whether ffmpeg will (efficiently?) read an endless concat over a unix socket or pipe
- finish fix for libp2p-bitswap unwrap when peer disconnects. i can just mechanically turn them into warnings, but i do want to look closer at how the ledger works first to see if there's more to it than the peer disconnecting right after connecting or after requesting a block.
- look for other solutions but.. one option for the segmentation is to make a first pass on everything to turn streams into segments, and then to use a segment-oriented pipeline to process those in ways which could include splitting them on non-keyframes to fit them into ipfs blocks.
- going to need an abstraction for "an encoding pipeline" soon, including
  - a way to pool them for many requests.. send separate files/segments through a single ffmpeg process and single container instance
  - durable and secure way to represent a specific tool and configuration
  - inputs: configured for either URL (with network enabled) or concat socket thing (with network disabled)
  - outputs: raw ipfs segments (the current focus), unixfs-based segments (for compatibility with large segments, with peertube-style hls, and for concatenating segments into single files for downloads or compatibility. 

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
  - would be great to do this via a new comment/tag in m3u while retaining the current filenames and directory structure, so the video is still playable if you download it off ipfs or view it through a gateway. using ipfs URIs directly in the playlist, for example, seems correct but would be useless in practice.
  - also think about, in the same m3u8 extension, indicating which segments contain keyframes. is there already a way to do this?
- add some api parts and try multiple streams, starting/stopping them dynamically
- stop even trying to parse mpegts, let the sandboxed ffmpeg do all our media splitting

