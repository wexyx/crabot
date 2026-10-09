#!/usr/bin/env bash
set -euo pipefail

binary=${1:?usage: check-linux-libraries.sh BINARY}
# Inspect only our freshly built, trusted release binary.
libraries=$(ldd "$binary")
printf '%s\n' "$libraries"
if printf '%s\n' "$libraries" | grep -Eq 'lib(ssl|crypto)\.so\.(1[. ]|[24-9])'; then
  echo 'Release must use system OpenSSL 3.' >&2
  exit 1
fi
versions=$(readelf --version-info "$binary")
# C++ runtimes are bundled and checked separately. libc remains a system library.
for limit in GLIBC_2.36; do
  prefix=${limit%_*}
  required=$(printf '%s\n' "$versions" | grep -Eo "${prefix}_[0-9]+(\.[0-9]+)*" | sort -Vu | tail -1 || true)
  if [[ -n $required && $(printf '%s\n%s\n' "$required" "$limit" | sort -V | tail -1) != "$limit" ]]; then
    echo "Release requires $required, exceeding Debian 12 baseline $limit." >&2
    exit 1
  fi
done
if printf '%s\n' "$libraries" | grep -Fq 'not found'; then
  echo 'Release has unresolved shared libraries.' >&2
  exit 1
fi
