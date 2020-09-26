#!/bin/sh
cargo run \
	-- \
	-G http://99.149.215.66:8080 \
	-g https://dweb.link \
	-r /dns4/ipfs.diode.zone/tcp/443/wss/p2p/QmPjtoXdQobBpWa2yS4rfmHVDoCbom2r2SMDTUa1Nk7kJ5 \
	-r /ip4/99.149.215.66/tcp/4001/p2p/QmPjtoXdQobBpWa2yS4rfmHVDoCbom2r2SMDTUa1Nk7kJ5 \
	-- \
	-i https://live.diode.zone/hls/eyesopod/index.m3u8 \
	-c copy
