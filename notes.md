notes & junk
---------------

randomly, the latest of late night shower thoughts... instead of adding hashes to the m3u8 (Adding redundancy, adding format-specific deps, etc) let's focus on making the playlist itself a better ipfs citizen. ideally the common and simple case of playlists under 1MB should be representable as a single block with both a directory node and a file node in it, and whatever other metadata we want later included in additional files. this could separate concerns between the definition of the segments, their location on ipfs or https or otherwise, and any strategies we have for re-generating blocks.

```
ffmpeg -safe 0 -protocol_whitelist file,concat,unix -f concat -i <(for i in {1..1000}; do echo file concat:`pwd`/s00001.ts\|`pwd`/s00002.ts; done) -c copy ~/out.mkv
```

- test whether ffmpeg will (efficiently?) read an endless concat over a unix socket or pipe
  - looks like no, it tries to read the whole file into RAM. but if we are using this like a pool for transcoding segments and it doesn't actually need to be infinite, perhaps an arbitrary medium-sized finite number of entries is fine, so long as we have multiple processes in each pipeline pool. this also implies that pipelines which take a stream input aren't pooled but segmented inputs can also be pooled.
  - works with ffprobe too of course, so we can use a finite repetitive concat file to make repeated connections to a unix socket and get segments to inspect
- finish fix for libp2p-bitswap unwrap when peer disconnects. i can just mechanically turn them into warnings, but i do want to look closer at how the ledger works first to see if there's more to it than the peer disconnecting right after connecting or after requesting a block.
- look for other solutions but.. one option for the segmentation is to make a first pass on everything to turn streams into segments, and then to use a segment-oriented pipeline to process those in ways which could include splitting them on non-keyframes to fit them into ipfs blocks.
- going to need an abstraction for "an encoding pipeline" soon, including
  - a way to pool them for many requests.. send separate files/segments through a single ffmpeg process and single container instance
  - durable and secure way to represent a specific tool and configuration
  - inputs: configured for either URL (with network enabled) or concat socket thing (with network disabled)
  - outputs: raw ipfs segments (the current focus), unixfs-based segments (for compatibility with large segments, with peertube-style hls, and for concatenating segments into single files for downloads or compatibility.

Big questions to answer:
- how does this interact with transcoding? seems like block transcoding is basically okay but we probably need to include a backtrack by one segment in the stream to be sure we get enough data to decode a complete frame. tempting to dive into h264 here but it would be great to avoid getting too low-level to make it easier to support new codecs.
- when can this server decide to discard blocks? combination of block usage info + which (trusted?) peers have requested it + timestamps? nah, that seems horribly unreliable. this really needs to integrate with feedback from long-term storage, which currently is via this pinning api. that means we really need to keep asynchronous track of pinning requests, using their completion as a signal to mark blocks as discardable from our RAM storage. this means we need to keep track of a possibly large but finite number of outstanding pinning requests, depending on how often we update the pinning (which is currently the same as the publish interval but could be slower). also.. pinning should be pretty fast now that we have the warmer dialed in. so maybe it's fine.
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
