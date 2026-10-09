#!/usr/bin/env bash
set -euo pipefail

target=${1:?usage: build-static-openssl.sh TARGET CONFIGURE INSTALL_DIR}
configure=${2:?missing absolute Configure path}
prefix=${3:?missing absolute installation directory}
case "$target" in
  x86_64-unknown-linux-gnu) openssl_target=linux-x86_64 ;;
  aarch64-unknown-linux-gnu) openssl_target=linux-aarch64 ;;
  *) echo "Unsupported OpenSSL build target: $target" >&2; exit 2 ;;
esac
[[ $configure == /* && -f $configure && $prefix == /* ]] || {
  echo 'Configure must exist and both paths must be absolute.' >&2
  exit 2
}

# Out-of-tree build: leave Cargo's immutable registry sources untouched.
mkdir -p "$prefix-build"
cd "$prefix-build"
"${OPENSSL_SRC_PERL:-perl}" "$configure" "$openssl_target" \
  "--prefix=$prefix" --libdir=lib --openssldir=/etc/ssl \
  no-shared no-module no-tests no-comp no-zlib no-zlib-dynamic \
  no-ssl3 no-md2 no-rc5 no-weak-ssl-ciphers no-camellia no-idea no-seed -fPIC
# openssl-src omits documentation/apps; use the same library-only targets as it.
make depend
make -j "${CARGO_BUILD_JOBS:-2}" build_libs
make install_dev
for file in include/openssl/ssl.h lib/libssl.a lib/libcrypto.a; do
  [[ -s $prefix/$file ]] || { echo "Missing OpenSSL build output: $prefix/$file" >&2; exit 1; }
done
