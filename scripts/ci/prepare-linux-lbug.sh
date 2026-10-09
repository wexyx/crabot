#!/usr/bin/env bash
set -euo pipefail
target=${1:?usage: prepare-linux-lbug.sh TARGET VERSION DESTINATION}
version=${2:?Ladybug version required}
destination=${3:?Destination required}
case "$target" in
  x86_64-unknown-linux-gnu) arch=x86_64;;
  aarch64-unknown-linux-gnu) arch=aarch64;;
  *) echo "Unsupported Linux target: $target" >&2; exit 2;;
esac
[[ $version =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo 'Invalid Ladybug version.' >&2; exit 2; }
stage=$(mktemp -d)
trap 'rm -rf -- "$stage"' EXIT
archive="liblbug-static-linux-$arch-compat.tar.gz"
curl --proto '=https' --tlsv1.2 -fsSL --retry 2 \
  "https://github.com/LadybugDB/ladybug/releases/download/v$version/$archive" -o "$stage/lbug.tar.gz"
mkdir "$stage/extracted"
tar -xzf "$stage/lbug.tar.gz" -C "$stage/extracted"
mkdir -p "$destination"
for file in liblbug.a lbug.h lbug.hpp; do
  [[ -s $stage/extracted/$file ]] || { echo "Missing Ladybug prebuilt file: $file" >&2; exit 1; }
done
for file in liblbug.a lbug.h lbug.hpp; do
  cp "$stage/extracted/$file" "$destination/$file"
done
printf 'Prepared Ladybug %s (%s, compat); source fallback disabled.\n' "$version" "$arch"
