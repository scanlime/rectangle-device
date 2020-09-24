FROM ubuntu:20.04 AS builder

# Install distro packages

WORKDIR /root

COPY docker/nodejs-current.x ./
RUN ./nodejs-current.x

COPY docker/install-yarn.sh ./
RUN ./install-yarn.sh

COPY docker/install-podman.sh ./
RUN ./install-podman.sh

COPY docker/install-build-deps.sh ./
RUN ./install-build-deps.sh

# Install rust

COPY docker/rustup-init ./
RUN ./rustup-init -y 2>&1
ENV PATH /root/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

# Compile rust dependencies using a skeleton crate, for faster docker rebuilds

WORKDIR /build
COPY docker/skeleton/ ./
COPY Cargo.lock ./
RUN cargo build --release 2>&1

# Compile workspace members separately, also for faster docker rebuilds

COPY player ./player
COPY docker/skeleton/Cargo.toml ./
RUN \
echo '[workspace]' >> Cargo.toml && \
echo 'members = [ "player" ]' >> Cargo.toml && \
cd player && cargo build --release -vv 2>&1

COPY blocks ./blocks
COPY docker/skeleton/Cargo.toml ./
RUN \
echo '[workspace]' >> Cargo.toml && \
echo 'members = [ "blocks" ]' >> Cargo.toml && \
cd blocks && cargo build --release 2>&1

COPY sandbox ./sandbox
COPY docker/skeleton/Cargo.toml ./
RUN \
echo '[workspace]' >> Cargo.toml && \
echo 'members = [ "sandbox" ]' >> Cargo.toml && \
cd sandbox && cargo build --release 2>&1

# Replace the skeleton with the real app and build it

COPY Cargo.toml ./
COPY src src
RUN cargo build --release --bins 2>&1
RUN cargo install --path=. --root=/usr 2>&1

# Configure podman and pre-download some images we expect to need

COPY docker/containers.conf /etc/containers/containers.conf
COPY docker/storage.conf /etc/containers/storage.conf

RUN podman pull docker.io/jrottenberg/ffmpeg:4.3.1-scratch38 2>&1

# Packaging the parts of this image we intend to keep

WORKDIR /
RUN tar chvf image.tar \
# App binaries
usr/bin/rectangle-device \
bin/ls \
bin/ldd \
bin/openssl \
# Pre-installed transcode container images
var/lib/containers \
# Podman container engine
usr/bin/podman \
usr/bin/conmon \
usr/bin/crun \
usr/sbin/runc \
usr/bin/nsenter \
etc/containers \
usr/share/containers \
# System data files
usr/share/zoneinfo \
usr/share/ca-certificates \
etc/ssl \
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

FROM scratch
WORKDIR /
COPY --from=builder /image/ /
ENTRYPOINT [ "/usr/bin/rectangle-device" ]

