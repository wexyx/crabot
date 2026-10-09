#!/usr/bin/env bash
set -euo pipefail

target=${1:?usage: build-linux-release.sh TARGET}
# Run in the official GCC 13 / Debian 12 image. No native dependency source build.
export DEBIAN_FRONTEND=noninteractive
apt-get update
apt-get install -y --no-install-recommends ca-certificates curl libssl-dev pkg-config patchelf
export CC=gcc CXX=g++
export CARGO_BUILD_JOBS=2
unset LBUG_BUILD_FROM_SOURCE LBUG_RUST_BUILD_FROM_SOURCE LBUG_SHARED
unset OPENSSL_DIR OPENSSL_ROOT_DIR OPENSSL_LIB_DIR OPENSSL_INCLUDE_DIR OPENSSL_STATIC
unset CMAKE_TOOLCHAIN_FILE
export OPENSSL_NO_VENDOR=1
"$CXX" --version
pkg-config --modversion openssl

# Download and link-check before Rust compilation; never silently fall back to
# lbug's incomplete source archive if a release asset is missing.
lbug_dir="$PWD/target/lbug-prebuilt"
bash scripts/ci/prepare-linux-lbug.sh "$target" "${LBUG_VERSION:?LBUG_VERSION is required}" "$lbug_dir"
export LBUG_LIBRARY_DIR="$lbug_dir" LBUG_INCLUDE_DIR="$lbug_dir"
"$CXX" -std=c++20 -I "$lbug_dir" -include lbug.hpp scripts/ci/cxx20-probe.cpp \
  -L "$lbug_dir" -Wl,--whole-archive -llbug -Wl,--no-whole-archive \
  -lssl -lcrypto -latomic -ldl -pthread -o target/crabot-lbug-probe
bash scripts/ci/check-linux-libraries.sh target/crabot-lbug-probe
target/crabot-lbug-probe

curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable
export PATH="$HOME/.cargo/bin:$PATH"
# The package contains the compiler's C++ runtime, not a replacement system libc.
export RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=-Wl,-rpath,\$ORIGIN/../lib"
cargo +stable build --release --locked -p agent-node
# lbug's external-library mode also emits an absolute build-directory RPATH.
# Replace it so the installed executable searches only its relocatable bundle.
patchelf --set-rpath '$ORIGIN/../lib' target/release/agent-node
bash scripts/ci/check-linux-libraries.sh target/release/agent-node
bash scripts/package-release.sh "$target"
