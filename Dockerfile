FROM ubuntu:20.04 AS builder

ENV PATH \
/home/builder/go/bin:\
/home/builder/.cargo/bin:\
/usr/local/sbin:\
/usr/local/bin:\
/usr/sbin:\
/usr/bin:\
/sbin:\
/bin

ENV DEBIAN_FRONTEND noninteractive
ENV DEBCONF_NONINTERACTIVE_SEEN true

# Compatibility with windows build hosts

RUN apt-get update && apt-get install -y dos2unix

# Install distro packages

WORKDIR /root

COPY docker/install/nodejs-current.sh ./
RUN dos2unix -q nodejs-current.sh; bash ./nodejs-current.sh

COPY docker/install/yarn.sh ./
RUN dos2unix -q yarn.sh; sh ./yarn.sh

COPY docker/install/build-deps.sh ./
RUN dos2unix -q build-deps.sh; sh ./build-deps.sh

# Make non-root users, switch to builder

RUN adduser builder --disabled-login </dev/null >/dev/null 2>/dev/null
RUN adduser rectangle-device --disabled-login </dev/null >/dev/null 2>/dev/null
USER builder:builder
WORKDIR /home/builder

# Build a fresh golang

FROM builder as golang

RUN \
git clone https://go.googlesource.com/go /home/builder/go 2>&1 && \
cd /home/builder/go && \
git checkout tags/go1.15.2 2>&1
RUN \
cd /home/builder/go/src && \
./all.bash

# Build latest crun from git

FROM builder as crun

RUN \
git clone https://github.com/containers/crun.git 2>&1
RUN \
cd crun && \
./autogen.sh && \
./configure --prefix=/usr && \
make

# Build latest conmon from git

FROM golang as conmon

RUN \
git clone https://github.com/containers/conmon 2>&1
RUN \
cd conmon && \
export GOCACHE="$(mktemp -d)" && \
make

# Build latest podman from git

FROM golang as podman

RUN \
git clone https://github.com/containers/podman/ /home/builder/go/src/github.com/containers/podman
RUN \
cd /home/builder/go/src/github.com/containers/podman && \
export GOPATH=/home/builder/go && \
make BUILDTAGS="selinux seccomp"
USER root
RUN \
cd /home/builder/go/src/github.com/containers/podman && \
make install PREFIX=/usr
USER builder:builder

# Install latest stable rust

FROM builder as rust

COPY --chown=builder docker/install/rustup-init.sh ./
RUN dos2unix -q rustup-init.sh; ./rustup-init.sh -y 2>&1

# Compile rust dependencies using a skeleton crate, for faster docker rebuilds

FROM rust as skeleton

COPY --chown=builder docker/skeleton/ ./
COPY --chown=builder Cargo.lock ./
RUN cargo build --release 2>&1

# Compile workspace members separately, also for faster docker rebuilds

COPY --chown=builder player player
COPY --chown=builder docker/skeleton/Cargo.toml ./
RUN \
echo '[workspace]' >> Cargo.toml && \
echo 'members = [ "player" ]' >> Cargo.toml && \
cd player && cargo build --release -vv 2>&1

COPY --chown=builder blocks blocks
COPY --chown=builder docker/skeleton/Cargo.toml ./
RUN \
echo '[workspace]' >> Cargo.toml && \
echo 'members = [ "blocks" ]' >> Cargo.toml && \
cd blocks && cargo build --release 2>&1

COPY --chown=builder sandbox sandbox
COPY --chown=builder docker/skeleton/Cargo.toml ./
RUN \
echo '[workspace]' >> Cargo.toml && \
echo 'members = [ "sandbox" ]' >> Cargo.toml && \
cd sandbox && cargo build --release 2>&1

# Replace the skeleton with the real app and build it

FROM skeleton as app
COPY --chown=builder Cargo.toml Cargo.toml
COPY --chown=builder src src
RUN cargo build --release --bins 2>&1

# Post-build install and configure, as root again

USER root
RUN install target/release/rectangle-device /usr/bin/rectangle-device

COPY --from=podman /usr/bin/podman /usr/bin/podman
COPY --from=crun /home/builder/crun/crun /usr/bin/crun
COPY --from=conmon /home/builder/conmon/bin/conmon /usr/bin/conmon

COPY docker/podman/containers.conf /etc/containers/containers.conf
COPY docker/podman/storage.conf /etc/containers/storage.conf

# Pull initial set of transcode images as the app user

USER rectangle-device
WORKDIR /home/rectangle-device

RUN podman pull docker.io/jrottenberg/ffmpeg:4.3.1-scratch38 2>&1

# Packaging the parts of this image we intend to keep

USER root
WORKDIR /
RUN tar chvf image.tar \
#
# App
usr/bin/rectangle-device \
home/rectangle-device \
#
# System tools
bin/ls \
bin/ldd \
bin/openssl \
#
# Podman container engine
usr/bin/podman \
usr/libexec/podman \
usr/bin/nsenter \
etc/containers \
usr/share/containers \
var/run/containers \
var/lib/containers \
#
# System data files
usr/share/zoneinfo \
usr/share/ca-certificates \
etc/ssl \
etc/passwd \
etc/group \
etc/shadow \
#
# Dynamic libraries, as needed
lib64 \
usr/lib64 \
lib/x86_64-linux-gnu/libc.so.6 \
lib/x86_64-linux-gnu/libm.so.6 \
lib/x86_64-linux-gnu/libtinfo.so.6 \
lib/x86_64-linux-gnu/libssl.so.1.1 \
lib/x86_64-linux-gnu/libcrypto.so.1.1 \
lib/x86_64-linux-gnu/libz.so.1 \
lib/x86_64-linux-gnu/libdl.so.2 \
lib/x86_64-linux-gnu/libpthread.so.0 \
lib/x86_64-linux-gnu/libgpgme.so.11 \
lib/x86_64-linux-gnu/libgcc_s.so.1 \
lib/x86_64-linux-gnu/libseccomp.so.2 \
lib/x86_64-linux-gnu/librt.so.1 \
lib/x86_64-linux-gnu/libassuan.so.0 \
lib/x86_64-linux-gnu/libgpg-error.so.0 \
lib/x86_64-linux-gnu/libyajl.so.2 \
lib/x86_64-linux-gnu/libsystemd.so.0 \
lib/x86_64-linux-gnu/liblzma.so.5 \
lib/x86_64-linux-gnu/liblz4.so.1 \
lib/x86_64-linux-gnu/libselinux.so.1 \
lib/x86_64-linux-gnu/libpcre2-8.so.0 \
lib/x86_64-linux-gnu/libgcrypt.so.20 \
lib/x86_64-linux-gnu/libglib-2.0.so.0 \
lib/x86_64-linux-gnu/libpcre.so.3

RUN \
mkdir image && \
cd image && \
tar xf ../image.tar && \
mkdir proc sys dev tmp var/tmp && \
chmod 01777 tmp var/tmp

WORKDIR /
FROM scratch
COPY --from=builder /image/ /

USER rectangle-device
ENTRYPOINT [ "/usr/bin/rectangle-device" ]

# Incoming libp2p connections
EXPOSE 4004/tcp
