#!/usr/bin/env bash
set -euo pipefail

target=${1:?usage: build-linux-release.sh TARGET}
export CC=gcc CXX=g++
# Build all native code against this image's glibc, not a newer prebuilt library.
export LBUG_BUILD_FROM_SOURCE=1
# Bound native compilation memory on hosted runners.
export CARGO_BUILD_JOBS=2 CMAKE_BUILD_PARALLEL_LEVEL=2
# Do not allow image defaults to override openssl-sys's vendored build.
export OPENSSL_NO_VENDOR=0
unset OPENSSL_DIR OPENSSL_LIB_DIR OPENSSL_INCLUDE_DIR
"$CXX" --version
"$CXX" -std=c++20 scripts/ci/cxx20-probe.cpp -o /tmp/crabot-cxx20-probe
/tmp/crabot-cxx20-probe

curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable
export PATH="$HOME/.cargo/bin:$PATH"
cargo +stable build --release --locked -p agent-node
bash scripts/ci/check-linux-libraries.sh target/release/agent-node
bash scripts/package-release.sh "$target"
