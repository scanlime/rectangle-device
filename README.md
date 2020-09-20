rectangle device
================

This is an experimental live + vod video server that integrates with IPFS for long-term distributed storage.

This is a proof of concept for streaming video ingest in a format that could be quickly shared via IPFS while the stream is still ongoing.

Build dependencies:
- rust
- yarn

Runtime dependencies:
- ffmpeg
- openssl

Big questions to answer:
- how does this interact with transcoding
- when can this server decide to discard blocks
- how do we represent instructions for transforming discardable blocks (docker compatible images?)

Need more tooling to debug this effectively. Are there libp2p dissector tools? Detailed tracing with timestamps?

Just code:
- segmentation right now is not okay
- templating for html
- js frontend updates live stream using pubsub
- use hashes in m3u playlist to remove a layer of indirection
- add some api parts and try multiple streams, starting/stopping them dynamically
