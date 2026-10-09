#!/usr/bin/env bash
set -euo pipefail
# Keep presentation on stderr: curl | bash works without reading from stdin,
# and CI / redirected output stays free of terminal escape sequences.
accent='' muted='' green='' red='' reset=''
interactive=false
if [[ -t 2 && ${TERM:-dumb} != dumb && -z ${NO_COLOR+x} ]]; then
  interactive=true
  accent=$'\033[1;36m'; muted=$'\033[2m'; green=$'\033[1;32m'; red=$'\033[1;31m'; reset=$'\033[0m'
fi
step() { printf '\n%s  [%s/5]%s %s\n' "$accent" "$1" "$reset" "$2" >&2; }
note() { printf '%s        %s%s\n' "$muted" "$1" "$reset" >&2; }
fail() { printf '\n%s  ERROR%s %s\n' "$red" "$reset" "$1" >&2; exit 1; }
trap 'code=$?; printf "\n%s  Installation stopped%s (exit %s). Review the error above and retry.\n" "$red" "$reset" "$code" >&2; exit "$code"' ERR
if $interactive; then
  printf '\n%s' "$accent" >&2
  printf '%s\n' \
    '   ██████╗██████╗  █████╗ ██████╗  ██████╗ ████████╗' \
    '  ██╔════╝██╔══██╗██╔══██╗██╔══██╗██╔═══██╗╚══██╔══╝' \
    '  ██║     ██████╔╝███████║██████╔╝██║   ██║   ██║' \
    '  ██║     ██╔══██╗██╔══██║██╔══██╗██║   ██║   ██║' \
    '  ╚██████╗██║  ██║██║  ██║██████╔╝╚██████╔╝   ██║' \
    '   ╚═════╝╚═╝  ╚═╝╚═╝  ╚═╝╚═════╝ ╚═════╝    ╚═╝' >&2
  printf '%s\n' "$reset" >&2
else
  printf '\n  CRABOT / INSTALLER\n' >&2
fi
version=${CRABOT_VERSION:-latest}
prefix=${CRABOT_INSTALL_PREFIX:-$HOME/.local}
repo=${CRABOT_REPOSITORY:-wexyx/crabot}
step 1 'Checking platform and release'
case "$(uname -s):$(uname -m)" in
  Darwin:arm64) target=aarch64-apple-darwin;;
  Darwin:x86_64) target=x86_64-apple-darwin;;
  Linux:x86_64) target=x86_64-unknown-linux-gnu;;
  Linux:aarch64|Linux:arm64) target=aarch64-unknown-linux-gnu;;
  *) fail 'Unsupported platform. Use a supported macOS/Linux release or build from source.';;
esac
[[ $prefix == /* && $prefix != / && $repo =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]] || fail 'Invalid install prefix or repository.'
[[ $version == latest || $version =~ ^v[0-9]+\.[0-9]+\.[0-9]+([.-][A-Za-z0-9.-]+)?$ ]] || fail 'Version must be latest or vX.Y.Z.'
for cmd in curl tar; do command -v "$cmd" >/dev/null || fail "Missing $cmd"; done
if ! command -v sha256sum >/dev/null && ! command -v shasum >/dev/null; then fail 'SHA-256 checker required.'; fi
base="https://github.com/$repo/releases"
if [[ $version == latest ]]; then
  resolved=$(curl --proto '=https' --tlsv1.2 -fsSL -o /dev/null -w '%{url_effective}' "$base/latest")
  version=${resolved##*/}
  [[ $version =~ ^v[0-9]+\.[0-9]+\.[0-9]+([.-][A-Za-z0-9.-]+)?$ ]] || fail 'No published release found.'
fi
note "$version / $target"
step 2 'Downloading release'
archive="crabot-$target.tar.gz"
stage=$(mktemp -d)
trap 'rm -rf -- "$stage"' EXIT
for file in "$archive" "$archive.sha256"; do
  progress=(-sS)
  if $interactive && [[ $file == "$archive" ]]; then progress=(--progress-bar); fi
  curl --proto '=https' --tlsv1.2 -fL "${progress[@]}" --retry 2 "$base/download/$version/$file" -o "$stage/$file" || fail 'Release asset unavailable; no installation changes made.'
done
step 3 'Verifying checksum and bundle'
expected=$(awk 'NR==1 {print $1}' "$stage/$archive.sha256")
[[ $expected =~ ^[a-fA-F0-9]{64}$ ]] || fail 'Invalid checksum file.'
if command -v sha256sum >/dev/null; then actual=$(sha256sum "$stage/$archive"); else actual=$(shasum -a 256 "$stage/$archive"); fi
[[ ${actual%% *} == "$expected" ]] || fail 'Checksum mismatch; refusing installation.'
# Only extract the bundle produced by package-release.sh.
tar -tzf "$stage/$archive" | awk '$0 !~ /^crabot\// || $0 ~ /(^|\/)\.\.(\/|$)/ {bad=1} END {exit bad}' || fail 'Unsafe archive paths.'
tar -tvzf "$stage/$archive" | awk 'substr($0,1,1)!="-" && substr($0,1,1)!="d" {bad=1} END {exit bad}' || fail 'Archive links/devices are not allowed.'
tar -xzf "$stage/$archive" -C "$stage"
[[ -x $stage/crabot/bin/crabot && -x $stage/crabot/libexec/agent-node && -f $stage/crabot/web/index.html && -f $stage/crabot/skills/system/management/management-guide/SKILL.md && -s $stage/crabot/lib/lbug/fts/libfts.lbug_extension ]] || fail 'Incomplete release bundle.'
note 'SHA-256 verified / archive paths checked'
step 4 'Installing command'
mkdir -p "$prefix/share/crabot/releases" "$prefix/bin"
destination=$(mktemp -d "$prefix/share/crabot/releases/$version-$target.XXXXXX")
cp -R "$stage/crabot/." "$destination/"
if [[ -e $prefix/bin/crabot && ! -L $prefix/bin/crabot ]]; then fail "Refusing to overwrite $prefix/bin/crabot; bundle saved at $destination"; fi
ln -sfn "$destination/bin/crabot" "$prefix/bin/crabot"
note "$prefix/bin/crabot"
step 5 'Configuring your shell'
# Install command discovery as part of installation, without replacing shell settings.
# A child script cannot change its parent shell; these entries apply to new terminals.
printf -v quoted_bin '%q' "$prefix/bin"
path_line="case \":\$PATH:\" in *:${quoted_bin}:*) ;; *) export PATH=${quoted_bin}:\"\$PATH\" ;; esac # Crabot PATH"
profiles=()
login_shell=${SHELL:-/bin/sh}
case "${login_shell##*/}" in
  zsh) profiles+=("${ZDOTDIR:-$HOME}/.zshrc") ;;
  bash)
    profiles+=("$HOME/.bashrc")
    if [[ -f $HOME/.bash_profile ]]; then profiles+=("$HOME/.bash_profile")
    elif [[ -f $HOME/.bash_login ]]; then profiles+=("$HOME/.bash_login")
    else profiles+=("$HOME/.profile"); fi ;;
  fish)
    fish_bin=${prefix//\\/\\\\}; fish_bin=${fish_bin//\'/\\\'}
    path_line="if not contains -- '$fish_bin/bin' \$PATH; set -gx PATH '$fish_bin/bin' \$PATH; end # Crabot PATH"
    profiles+=("${XDG_CONFIG_HOME:-$HOME/.config}/fish/conf.d/crabot.fish") ;;
  *) profiles+=("$HOME/.profile") ;;
esac
for profile in "${profiles[@]}"; do
  if [[ -f $profile ]] && grep -Fqx -- "$path_line" "$profile"; then continue; fi
  mkdir -p "$(dirname -- "$profile")"
  if [[ -f $profile ]]; then
    backup=$(mktemp "$profile.crabot-backup.XXXXXX")
    cp -p "$profile" "$backup"
    note "Shell configuration backup: $backup"
  fi
  (umask 077; printf '\n%s\n' "$path_line" >> "$profile")
  note "Configured PATH: $profile"
done
printf '\n%s  READY%s  Crabot %s\n' "$green" "$reset" "$version" >&2
note "Installed to $destination"
printf '\n  Open a new terminal and run:\n\n%s    crabot%s\n\n' "$accent" "$reset" >&2
case ":$PATH:" in *":$prefix/bin:"*) note 'You can also run crabot in this terminal.';; *) printf -v launch '%q' "$prefix/bin/crabot"; note "Run now: $launch";; esac
note 'Next update: /update inside Crabot, then restart.'
