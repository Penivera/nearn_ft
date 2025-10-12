#!/bin/bash
# build.sh

# Exit immediately if a command fails
set -e

echo "--- Installing System Dependencies ---"
# Use sudo to install packages in Railway's build environment
sudo apt-get update -y && sudo apt-get install -y \
  libudev-dev \
  clang \
  libssl-dev \
  pkg-config \
  perl \
  curl

echo "--- Building Rust Application ---"
cargo build --release

echo "--- Build Complete ---"