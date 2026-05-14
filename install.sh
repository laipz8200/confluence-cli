#!/bin/sh
set -eu

BIN_NAME="confluence-cli"
REPO="${CONFLUENCE_CLI_REPO:-laipz8200/confluence-cli}"
GITHUB_BASE_URL="${CONFLUENCE_CLI_GITHUB_BASE_URL:-https://github.com}"
GITHUB_API_URL="${CONFLUENCE_CLI_GITHUB_API_URL:-https://api.github.com}"
REQUESTED_VERSION="${CONFLUENCE_CLI_VERSION:-latest}"

GITHUB_BASE_URL="${GITHUB_BASE_URL%/}"
GITHUB_API_URL="${GITHUB_API_URL%/}"

fail() {
  printf 'confluence-cli install: %s\n' "$*" >&2
  exit 1
}

need_command() {
  command -v "$1" >/dev/null 2>&1 || fail "Missing required command: $1"
}

detect_target() {
  os=$(uname -s)
  arch=$(uname -m)

  case "$os:$arch" in
    Linux:x86_64 | Linux:amd64)
      printf 'x86_64-unknown-linux-gnu\n'
      ;;
    Linux:aarch64 | Linux:arm64)
      printf 'aarch64-unknown-linux-gnu\n'
      ;;
    Darwin:x86_64 | Darwin:amd64)
      printf 'x86_64-apple-darwin\n'
      ;;
    Darwin:arm64 | Darwin:aarch64)
      printf 'aarch64-apple-darwin\n'
      ;;
    *)
      fail "Unsupported platform: $os/$arch. Release binaries are available for Linux x86_64, Linux arm64, macOS x86_64, and macOS arm64."
      ;;
  esac
}

resolve_tag() {
  if [ "$REQUESTED_VERSION" = "latest" ]; then
    latest_json=$(curl -fsSL "$GITHUB_API_URL/repos/$REPO/releases/latest") \
      || fail "Failed to resolve the latest GitHub release."
    tag=$(printf '%s\n' "$latest_json" \
      | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' \
      | head -n 1)
    [ -n "$tag" ] || fail "Could not find a tag_name in the latest GitHub release response."
    printf '%s\n' "$tag"
    return
  fi

  case "$REQUESTED_VERSION" in
    v*) printf '%s\n' "$REQUESTED_VERSION" ;;
    *) printf 'v%s\n' "$REQUESTED_VERSION" ;;
  esac
}

default_install_dir() {
  if [ -n "${CONFLUENCE_CLI_INSTALL_DIR:-}" ]; then
    printf '%s\n' "$CONFLUENCE_CLI_INSTALL_DIR"
    return
  fi

  [ -n "${HOME:-}" ] || fail "HOME is not set. Set CONFLUENCE_CLI_INSTALL_DIR to choose an install directory."
  printf '%s/.local/bin\n' "$HOME"
}

need_command curl
need_command head
need_command install
need_command mktemp
need_command sed
need_command tar
need_command uname

target="${CONFLUENCE_CLI_TARGET:-$(detect_target)}"
tag=$(resolve_tag)
version="${tag#v}"
asset="confluence-cli-$version-$target.tar.gz"
package="confluence-cli-$version-$target"
url="$GITHUB_BASE_URL/$REPO/releases/download/$tag/$asset"
install_dir=$(default_install_dir)

tmpdir=$(mktemp -d "${TMPDIR:-/tmp}/confluence-cli.XXXXXX") \
  || fail "Failed to create a temporary directory."
cleanup() {
  rm -rf "$tmpdir"
}
trap cleanup EXIT INT TERM

archive="$tmpdir/$asset"
printf 'Downloading %s\n' "$url"
curl -fsSL -o "$archive" "$url" || fail "Failed to download $asset."

tar -xzf "$archive" -C "$tmpdir" || fail "Failed to extract $asset."
binary="$tmpdir/$package/$BIN_NAME"
[ -f "$binary" ] || fail "Release archive did not contain $package/$BIN_NAME."

mkdir -p "$install_dir" || fail "Failed to create install directory: $install_dir"
install -m 0755 "$binary" "$install_dir/$BIN_NAME" \
  || fail "Failed to install $BIN_NAME to $install_dir."

printf 'Installed %s to %s\n' "$BIN_NAME" "$install_dir/$BIN_NAME"
case ":$PATH:" in
  *":$install_dir:"*) ;;
  *) printf 'Note: %s is not on PATH.\n' "$install_dir" ;;
esac

printf 'Running %s config init\n' "$install_dir/$BIN_NAME"
if [ -t 0 ]; then
  "$install_dir/$BIN_NAME" config init \
    || fail "Failed to run $BIN_NAME config init."
elif ( : </dev/tty ) 2>/dev/null; then
  "$install_dir/$BIN_NAME" config init </dev/tty \
    || fail "Failed to run $BIN_NAME config init."
else
  "$install_dir/$BIN_NAME" config init \
    || fail "Failed to run $BIN_NAME config init."
fi
