FROM ubuntu:20.04 AS builder

WORKDIR /root

COPY docker/nodejs-current.x ./
RUN ./nodejs-current.x

COPY docker/install-yarn.sh ./
RUN ./install-yarn.sh

COPY docker/rustup-init ./
RUN ./rustup-init -y && ln -s /root/.cargo/bin/* /usr/bin/

COPY docker/install-podman.sh ./
RUN ./install-podman.sh

COPY docker/install-build-deps.sh ./
RUN ./install-build-deps.sh

WORKDIR /build

# Compile rust dependencies separately (for faster docker rebuilds)

COPY docker/skeleton/ ./
COPY Cargo.lock ./
RUN cargo build --release 2>&1

# Compile workspace members separately (for faster docker rebuilds)

COPY player ./player
COPY docker/skeleton/Cargo.toml ./
RUN echo '[workspace]' >> Cargo.toml && \ 
    echo 'members = [ "player" ]' >> Cargo.toml && \
    cd player && cargo build --release -vv 2>&1

COPY sandbox ./sandbox
COPY docker/skeleton/Cargo.toml ./
RUN echo '[workspace]' >> Cargo.toml && \ 
    echo 'members = [ "sandbox" ]' >> Cargo.toml && \
    cd sandbox && cargo build --release -vv 2>&1

COPY blocks ./blocks
COPY docker/skeleton/Cargo.toml ./
RUN echo '[workspace]' >> Cargo.toml && \ 
    echo 'members = [ "blocks" ]' >> Cargo.toml && \
    cd blocks && cargo build --release -vv 2>&1

# Build the rest

COPY . .
RUN cargo build --release -vv 2>&1

# Now assemble a minimal linux image
FROM scratch
WORKDIR /

# glibc
COPY --from=builder /lib/x86_64-linux-gnu/libc.so.6 /lib/x86_64-linux-gnu/
COPY --from=builder /lib64 /lib64

