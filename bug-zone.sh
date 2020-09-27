#!/bin/sh

if [ "$1" = "docker" ]; then
  cmd="docker run `docker build . -q`"
else
  cmd="cargo run --"
fi

$cmd \
  --interval 5s \
  --pin-gw http://99.149.215.66:8080 \
  --pub-gw https://dweb.link \
  --router /ip4/99.149.215.66/tcp/4001/p2p/QmPjtoXdQobBpWa2yS4rfmHVDoCbom2r2SMDTUa1Nk7kJ5 \
  -- \
  -i https://live.diode.zone/hls/eyesopod/index.m3u8 \
  -c copy

