#!/usr/bin/env bash
set -euo pipefail

binary=${1:?usage: check-linux-libraries.sh BINARY}
# Inspect only our freshly built, trusted release binary.
libraries=$(ldd "$binary")
printf '%s\n' "$libraries"
if printf '%s\n' "$libraries" | grep -Eq 'lib(ssl|crypto)\.so'; then
  echo 'Release must not depend on shared OpenSSL; check Cargo TLS features.' >&2
  exit 1
fi
versions=$(readelf --version-info "$binary")
# Debian 10 provides glibc 2.28 and GCC 8's C++ ABI. A newer compiler is fine,
# but its output must not require newer runtime symbols.
for limit in GLIBC_2.28 GLIBCXX_3.4.25 CXXABI_1.3.11; do
  prefix=${limit%_*}
  required=$(printf '%s\n' "$versions" | grep -Eo "${prefix}_[0-9]+(\.[0-9]+)*" | sort -Vu | tail -1 || true)
  if [[ -n $required && $(printf '%s\n%s\n' "$required" "$limit" | sort -V | tail -1) != "$limit" ]]; then
    echo "Release requires $required, exceeding Debian 10 baseline $limit." >&2
    exit 1
  fi
done
if printf '%s\n' "$libraries" | grep -Fq 'not found'; then
  echo 'Release has unresolved shared libraries.' >&2
  exit 1
fi
