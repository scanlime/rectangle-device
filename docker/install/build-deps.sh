#!/bin/sh
set -e
apt-get update
apt-get install -y \
  build-essential \
  git \
  pkg-config \
  openssl \
  libssl-dev \
  strace \
  gdb \
  nano \
  vim \
  git \
  golang-go \
  go-md2man \
  iptables \
  libapparmor-dev \
  libassuan-dev \
  libbtrfs-dev \
  libc6-dev \
  libdevmapper-dev \
  libglib2.0-dev \
  libgpgme-dev \
  libgpg-error-dev \
  libprotobuf-dev \
  libseccomp-dev \
  libselinux1-dev \
  libsystemd-dev \
  pkg-config \
  uidmap

