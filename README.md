rust-ipfs-toy
=============

```
fmpeg -loglevel panic -i https://live.diode.zone/hls/eyesopod/index.m3u8 -f mpegts - | \
  RUST_LOG=info cargo run 2> stderr.txt
```
