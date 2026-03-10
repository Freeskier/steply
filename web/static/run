#!/bin/sh
set -eu

REPO="Freeskier/steply"
VERSION="${STEPLY_VERSION:-latest}"

need_cmd() {
  command -v "$1" >/dev/null 2>&1
}

download() {
  url="$1"
  out="$2"
  if need_cmd curl; then
    curl -fsSL "$url" -o "$out"
    return
  fi
  if need_cmd wget; then
    wget -qO "$out" "$url"
    return
  fi
  echo "error: need curl or wget to download Steply" >&2
  exit 1
}

release_url() {
  asset="$1"
  if [ "$VERSION" = "latest" ]; then
    printf '%s\n' "https://github.com/$REPO/releases/latest/download/$asset"
  else
    printf '%s\n' "https://github.com/$REPO/releases/download/$VERSION/$asset"
  fi
}

detect_asset() {
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Linux)
      case "$arch" in
        x86_64|amd64) printf '%s\n' "steply-x86_64-unknown-linux-gnu.tar.gz" ;;
        *)
          echo "error: unsupported Linux architecture: $arch" >&2
          exit 1
          ;;
      esac
      ;;
    Darwin)
      case "$arch" in
        arm64|aarch64) printf '%s\n' "steply-aarch64-apple-darwin.tar.gz" ;;
        *)
          echo "error: unsupported macOS architecture: $arch" >&2
          exit 1
          ;;
      esac
      ;;
    *)
      echo "error: unsupported operating system: $os" >&2
      echo "hint: Windows users should run run.ps1" >&2
      exit 1
      ;;
  esac
}

tmpdir="$(mktemp -d "${TMPDIR:-/tmp}/steply-run.XXXXXX")"
trap 'rm -rf "$tmpdir"' EXIT INT TERM

asset="$(detect_asset)"
archive="$tmpdir/$asset"
url="$(release_url "$asset")"

download "$url" "$archive"
tar -xzf "$archive" -C "$tmpdir"
chmod +x "$tmpdir/steply"
exec "$tmpdir/steply" "$@"
