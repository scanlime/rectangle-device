FROM ubuntu:20.04 AS base

# Download and install dependencies

WORKDIR /root

COPY docker/nodejs-current.x ./
RUN ./nodejs-current.x

COPY docker/install-yarn.sh ./
RUN ./install-yarn.sh

COPY docker/rustup-init ./
RUN ./rustup-init -y 2>&1 && \
    ln -s /root/.cargo/bin/* /usr/bin/

COPY docker/install-podman.sh ./
RUN ./install-podman.sh

COPY docker/install-build-deps.sh ./
RUN ./install-build-deps.sh

FROM base as builder

# Compile rust dependencies separately (for faster docker rebuilds)

WORKDIR /build

COPY docker/skeleton/ ./
COPY Cargo.lock ./
RUN cargo build --release 2>&1

# Compile workspace members separately (for faster docker rebuilds)

COPY player ./player
COPY docker/skeleton/Cargo.toml ./
RUN echo '[workspace]' >> Cargo.toml && \ 
    echo 'members = [ "player" ]' >> Cargo.toml && \
    cd player && cargo build --release -vv 2>&1

COPY blocks ./blocks
COPY docker/skeleton/Cargo.toml ./
RUN echo '[workspace]' >> Cargo.toml && \ 
    echo 'members = [ "blocks" ]' >> Cargo.toml && \
    cd blocks && cargo build --release 2>&1

COPY sandbox ./sandbox
COPY docker/skeleton/Cargo.toml ./
RUN echo '[workspace]' >> Cargo.toml && \ 
    echo 'members = [ "sandbox" ]' >> Cargo.toml && \
    cd sandbox && cargo build --release 2>&1

# Build the rest

COPY Cargo.toml ./
COPY src ./
RUN cargo build --release --bins 2>&1

# Now assemble a minimal linux image

FROM scratch

# System libraries as-needed

WORKDIR /
COPY --from=base /lib64 ./

WORKDIR /lib/x86_64-linux-gnu
COPY --from=base /lib/x86_64-linux-gnu/libc.so.6 ./
COPY --from=base /lib/x86_64-linux-gnu/libm.so.6 ./
COPY --from=base /lib/x86_64-linux-gnu/libssl.so.1.1 ./
COPY --from=base /lib/x86_64-linux-gnu/libcrypto.so.1.1 ./
COPY --from=base /lib/x86_64-linux-gnu/libz.so.1 ./
COPY --from=base /lib/x86_64-linux-gnu/libdl.so.2 ./
COPY --from=base /lib/x86_64-linux-gnu/libpthread.so.0 ./
COPY --from=base /lib/x86_64-linux-gnu/libgcc_s.so.1 ./

# Install built app last

COPY --from=builder /build/target/release/rectangle-device /usr/bin/
ENTRYPOINT [ "/usr/bin/rectangle-device" ]

