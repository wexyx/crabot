#!/usr/bin/env bash
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"
target=${1:?usage: package-release.sh TARGET [BINARY]}
[[ $target =~ ^[a-zA-Z0-9_-]+$ ]] || exit 2
binary=${2:-target/release/agent-node}
[[ -x $binary && -f apps/web/dist/index.html ]] || { echo 'Build Rust and Web first.' >&2; exit 1; }
if [[ -n ${CRABOT_RELEASE_VERSION:-} ]]; then
  expected="Crabot v${CRABOT_RELEASE_VERSION#v}"
  actual=$("$binary" --version)
  [[ $actual == "$expected" ]] || { echo "Version mismatch: expected $expected, got $actual. Rebuild with CRABOT_RELEASE_VERSION." >&2; exit 1; }
fi
stage=$(mktemp -d)
trap 'rm -rf -- "$stage"' EXIT
mkdir -p "$stage/crabot/bin" "$stage/crabot/libexec" dist
cp "$binary" "$stage/crabot/libexec/agent-node"
cp install.sh "$stage/crabot/libexec/install.sh"
cp scripts/release/crabot "$stage/crabot/bin/crabot"
chmod 755 "$stage/crabot/bin/crabot"
cp -R apps/web/dist "$stage/crabot/web"
cp README.md "$stage/crabot/README.md"
cp LICENSE "$stage/crabot/LICENSE"
cp CONTRIBUTING.md "$stage/crabot/CONTRIBUTING.md"
cp -R docs "$stage/crabot/docs"
cp -R conf "$stage/crabot/conf"
mkdir -p "$stage/crabot/skills"
tar --exclude=.runtime --exclude=node_modules -C skills -cf - system | tar -C "$stage/crabot/skills" -xf -
case "$target" in
  aarch64-apple-darwin) lbug_platform=osx_arm64;;
  x86_64-apple-darwin) lbug_platform=osx_amd64;;
  x86_64-unknown-linux-gnu) lbug_platform=linux_amd64;;
  aarch64-unknown-linux-gnu) lbug_platform=linux_arm64;;
  *) echo "Unsupported Ladybug extension target: $target" >&2; exit 2;;
esac
lbug_version=$(awk '
  $0 == "name = \"lbug\"" {found=1; next}
  found && $1 == "version" {gsub(/\"/, "", $3); print $3; exit}
' Cargo.lock)
[[ $lbug_version =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo 'Cannot resolve lbug version from Cargo.lock.' >&2; exit 1; }
fts_dest="$stage/crabot/lib/lbug/fts/libfts.lbug_extension"
mkdir -p "$(dirname -- "$fts_dest")"
if [[ -n ${CRABOT_LBUG_FTS_EXTENSION:-} ]]; then
  [[ -f $CRABOT_LBUG_FTS_EXTENSION ]] || { echo "CRABOT_LBUG_FTS_EXTENSION does not exist: $CRABOT_LBUG_FTS_EXTENSION" >&2; exit 1; }
  cp "$CRABOT_LBUG_FTS_EXTENSION" "$fts_dest"
elif [[ -n ${CRABOT_LBUG_EXTENSION_DIR:-} && -f $CRABOT_LBUG_EXTENSION_DIR/$lbug_version/$lbug_platform/fts/libfts.lbug_extension ]]; then
  cp "$CRABOT_LBUG_EXTENSION_DIR/$lbug_version/$lbug_platform/fts/libfts.lbug_extension" "$fts_dest"
elif [[ -f $HOME/.lbdb/extension/$lbug_version/$lbug_platform/fts/libfts.lbug_extension ]]; then
  cp "$HOME/.lbdb/extension/$lbug_version/$lbug_platform/fts/libfts.lbug_extension" "$fts_dest"
else
  command -v curl >/dev/null || { echo 'curl is required to package Ladybug FTS extension.' >&2; exit 1; }
  curl --proto '=https' --tlsv1.2 -fSL --retry 2 \
    "https://extension.ladybugdb.com/v$lbug_version/$lbug_platform/fts/libfts.lbug_extension" \
    -o "$fts_dest"
fi
[[ -s $fts_dest ]] || { echo 'Ladybug FTS extension is empty or missing.' >&2; exit 1; }
chmod 644 "$fts_dest"
if [[ $target == *-unknown-linux-gnu ]]; then
  # GCC 13 headers are needed by lbug.hpp; don't require users to upgrade GCC.
  for library in libstdc++.so.6 libgcc_s.so.1; do
    source=$("${CXX:-g++}" -print-file-name="$library")
    [[ $source == /* && -f $source ]] || { echo "Cannot locate $library" >&2; exit 1; }
    cp -L "$source" "$stage/crabot/lib/$library"
  done
  gcc_version=$("${CXX:-g++}" -dumpfullversion)
  [[ $gcc_version =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo 'Cannot resolve GCC runtime license version.' >&2; exit 1; }
  mkdir -p "$stage/crabot/licenses/gcc"
  for license in COPYING3 COPYING.RUNTIME; do
    curl --proto '=https' --tlsv1.2 -fsSL --retry 2 \
      "https://raw.githubusercontent.com/gcc-mirror/gcc/releases/gcc-$gcc_version/$license" \
      -o "$stage/crabot/licenses/gcc/$license"
  done
  # Also resolve dependencies of dlopened FTS and the bundled C++ runtimes.
  export LD_LIBRARY_PATH="$stage/crabot/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
  for library in libstdc++.so.6 libgcc_s.so.1; do
    bash scripts/ci/check-linux-libraries.sh "$stage/crabot/lib/$library"
  done
  bash scripts/ci/check-linux-libraries.sh "$stage/crabot/libexec/agent-node"
  bash scripts/ci/check-linux-libraries.sh "$fts_dest"
fi
archive="crabot-$target.tar.gz"
tar -czf "dist/$archive" -C "$stage" crabot
cd dist
if command -v sha256sum >/dev/null; then sha256sum "$archive" > "$archive.sha256"; else shasum -a 256 "$archive" > "$archive.sha256"; fi
printf 'Created dist/%s\n' "$archive"
