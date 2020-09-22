rectangle-device-player
=======================

This is the web client that is automatically delivered in each "player" block that accompanies a new playlist.

The player itself is delivered over a public IPFS gateway, but the code within the player is a proof-of-concept testbed for delivering the video itself solely over js-ipfs. In a more real scenario, IPFS would be combined with direct connections between the client and this daemon or other reachable https servers.

