#!/bin/sh
set -e
apt-get update

# These are actually needed to compile currently
apt-get install -y build-essential git pkg-config openssl libssl-dev

# These are just for developer convenience
apt-get install -y strace gdb

