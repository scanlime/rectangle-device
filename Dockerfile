# this is the latest ubuntu supported by the podman distribution we're using.
# to do: build an image containing only the necessary binaries, not the whole build env.
FROM ubuntu:20.04

WORKDIR /root

COPY docker/install-yarn.sh ./
RUN ./install-yarn.sh

COPY docker/rustup-init ./
RUN ./rustup-init -y && ln -s /root/.cargo/bin/* /usr/bin/

COPY docker/install-podman.sh ./
RUN ./install-podman.sh

COPY docker/install-build-deps.sh ./
RUN ./install-build-deps.sh

WORKDIR /build

COPY . .
RUN cargo build --release

