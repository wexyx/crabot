#!/usr/bin/env bash
set -euo pipefail

target=${1:?usage: build-linux-release.sh TARGET}
# manylinux's minimal Perl installation is not a complete OpenSSL build runtime.
# Install modules explicitly in this disposable CI container, never on user hosts.
dnf install -y perl-core perl-IPC-Cmd make
export PERL=/usr/bin/perl OPENSSL_SRC_PERL=/usr/bin/perl
"$OPENSSL_SRC_PERL" -MIPC::Cmd -MFindBin -MFile::Compare -MFile::Copy -MText::ParseWords -e 'print "OpenSSL Perl modules: OK\n"'
export CC=gcc CXX=g++
# Build all native code against this image's glibc, not a newer prebuilt library.
export LBUG_BUILD_FROM_SOURCE=1
# Bound native compilation memory on hosted runners.
export CARGO_BUILD_JOBS=2 CMAKE_BUILD_PARALLEL_LEVEL=2
# Rust and LadybugDB will use one static OpenSSL built below from locked sources.
unset OPENSSL_DIR OPENSSL_ROOT_DIR OPENSSL_LIB_DIR OPENSSL_INCLUDE_DIR
"$CXX" --version
"$CXX" -std=c++20 scripts/ci/cxx20-probe.cpp -o /tmp/crabot-cxx20-probe
/tmp/crabot-cxx20-probe

curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable
export PATH="$HOME/.cargo/bin:$PATH"
cargo +stable fetch --locked --target "$target"
# Compile-check the exact vendored Configure script, including its BEGIN imports,
# before spending time compiling Rust and LadybugDB. No OpenSSL source is modified.
openssl_configure=$(cargo +stable metadata --locked --offline --format-version 1 --filter-platform "$target" |
  /opt/python/cp311-cp311/bin/python -c 'import json, pathlib, sys
packages = [p for p in json.load(sys.stdin)["packages"] if p["name"] == "openssl-src"]
if len(packages) != 1:
    sys.exit("Expected exactly one locked openssl-src package")
print(pathlib.Path(packages[0]["manifest_path"]).parent / "openssl" / "Configure")')
"$OPENSSL_SRC_PERL" -c "$openssl_configure"
export OPENSSL_DIR="$PWD/target/native-openssl/install"
bash scripts/ci/build-static-openssl.sh "$target" "$openssl_configure" "$OPENSSL_DIR"
export OPENSSL_ROOT_DIR="$OPENSSL_DIR" OPENSSL_STATIC=1 OPENSSL_NO_VENDOR=1
export CMAKE_TOOLCHAIN_FILE="$PWD/scripts/ci/linux-release-toolchain.cmake"
cmake -S scripts/ci/openssl-probe -B target/openssl-probe \
  -DCMAKE_TOOLCHAIN_FILE="$CMAKE_TOOLCHAIN_FILE"
cmake --build target/openssl-probe --parallel 2
target/openssl-probe/crabot-openssl-probe
bash scripts/ci/check-linux-libraries.sh target/openssl-probe/crabot-openssl-probe
cargo +stable build --release --locked -p agent-node
bash scripts/ci/check-linux-libraries.sh target/release/agent-node
bash scripts/package-release.sh "$target"
