FROM ubuntu:20.04

WORKDIR /root

COPY docker/install-yarn.sh .
RUN ./install-yarn.sh

COPY docker/rustup-init .
RUN ./rustup-init -y && ln -s /root/.cargo/bin/* /usr/bin/

COPY docker/install-podman.sh .
RUN ./install-podman.sh

RUN apt-get install -y \
	build-essential pkg-config openssl libssl-dev

WORKDIR /build
COPY . .
RUN cargo build

