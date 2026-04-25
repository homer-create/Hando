#!/usr/bin/env bash
set -euo pipefail

VERSION="${NODE_VERSION:-v20.11.1}"

# Detect host triple — prefer rustc, fall back to uname
if command -v rustc &>/dev/null; then
  HOST_TRIPLE="$(rustc -vV | awk '/host:/ { print $2 }')"
else
  _os="$(uname -s)"
  _arch="$(uname -m)"
  case "$_os" in
    Darwin)
      [[ "$_arch" == "arm64" ]] && HOST_TRIPLE="aarch64-apple-darwin" || HOST_TRIPLE="x86_64-apple-darwin" ;;
    Linux)
      HOST_TRIPLE="x86_64-unknown-linux-gnu" ;;
    MINGW*|MSYS*|CYGWIN*)
      HOST_TRIPLE="x86_64-pc-windows-msvc" ;;
    *)
      echo "Cannot detect host triple; set HOST_TRIPLE env var and re-run."; exit 1 ;;
  esac
fi

BIN_DIR="$(cd "$(dirname "$0")/../src-tauri/binaries" && pwd)"
mkdir -p "$BIN_DIR"

# Skip if binary already present
[[ -f "$BIN_DIR/node-${HOST_TRIPLE}.exe" || -f "$BIN_DIR/node-${HOST_TRIPLE}" ]] && {
  echo "Node binary already present, skipping download."
  exit 0
}

download_tgz() {
  local triple="$1" platform="$2" arch="$3"
  local url="https://nodejs.org/dist/${VERSION}/node-${VERSION}-${platform}-${arch}.tar.gz"
  echo "Fetching $url"
  curl -fsSL "$url" | tar -xz -C "$BIN_DIR" --strip-components=2 "node-${VERSION}-${platform}-${arch}/bin/node"
  mv "$BIN_DIR/node" "$BIN_DIR/node-${triple}"
  chmod +x "$BIN_DIR/node-${triple}"
}

download_zip() {
  local triple="$1" arch="$2"
  local url="https://nodejs.org/dist/${VERSION}/node-${VERSION}-win-${arch}.zip"
  echo "Fetching $url"
  local tmp
  tmp="$(mktemp -d)"
  curl -fsSL -o "$tmp/node.zip" "$url"
  unzip -q -d "$tmp" "$tmp/node.zip"
  cp "$tmp"/node-*/node.exe "$BIN_DIR/node-${triple}.exe"
  rm -rf "$tmp"
}

case "$HOST_TRIPLE" in
  x86_64-apple-darwin)      download_tgz "$HOST_TRIPLE" darwin x64 ;;
  aarch64-apple-darwin)     download_tgz "$HOST_TRIPLE" darwin arm64 ;;
  x86_64-pc-windows-msvc)   download_zip "$HOST_TRIPLE" x64 ;;
  x86_64-unknown-linux-gnu) download_tgz "$HOST_TRIPLE" linux x64 ;;
  *) echo "Unsupported host triple: $HOST_TRIPLE"; exit 1 ;;
esac

echo "Done. Host binary at $BIN_DIR/node-$HOST_TRIPLE"
