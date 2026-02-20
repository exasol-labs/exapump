#!/bin/sh
set -eu

REPO="exasol-labs/exapump"
INSTALL_DIR="${EXAPUMP_INSTALL_DIR:-$HOME/.local/bin}"

detect_os() {
  case "$(uname -s)" in
    Linux*)  echo "linux" ;;
    Darwin*) echo "macos" ;;
    *)       printf "Error: unsupported OS: %s\n" "$(uname -s)" >&2; exit 1 ;;
  esac
}

detect_arch() {
  case "$(uname -m)" in
    x86_64|amd64)  echo "x86_64" ;;
    aarch64|arm64) echo "aarch64" ;;
    *)             printf "Error: unsupported architecture: %s\n" "$(uname -m)" >&2; exit 1 ;;
  esac
}

get_latest_version() {
  url="https://api.github.com/repos/${REPO}/releases/latest"
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$url" 2>/dev/null
  elif command -v wget >/dev/null 2>&1; then
    wget -qO- "$url" 2>/dev/null
  else
    printf "Error: neither curl nor wget found\n" >&2
    exit 1
  fi | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"v\([^"]*\)".*/\1/p'
}

download() {
  url="$1"
  output="$2"
  if command -v curl >/dev/null 2>&1; then
    curl -fSL -o "$output" "$url"
  elif command -v wget >/dev/null 2>&1; then
    wget -q -O "$output" "$url"
  else
    printf "Error: neither curl nor wget found\n" >&2
    exit 1
  fi
}

main() {
  os="$(detect_os)"
  arch="$(detect_arch)"
  version="${EXAPUMP_VERSION:-$(get_latest_version)}"

  if [ -z "$version" ]; then
    printf "Error: could not determine latest version\n" >&2
    exit 1
  fi

  asset="exapump-${os}-${arch}"
  url="https://github.com/${REPO}/releases/download/v${version}/${asset}"

  printf "Installing exapump v%s (%s/%s)...\n" "$version" "$os" "$arch"

  tmpfile="$(mktemp)"
  trap 'rm -f "$tmpfile"' EXIT

  download "$url" "$tmpfile"

  mkdir -p "$INSTALL_DIR"
  mv "$tmpfile" "${INSTALL_DIR}/exapump"
  chmod +x "${INSTALL_DIR}/exapump"

  printf "Installed exapump to %s/exapump\n" "$INSTALL_DIR"

  case ":${PATH}:" in
    *":${INSTALL_DIR}:"*) ;;
    *)
      printf "\nAdd %s to your PATH:\n" "$INSTALL_DIR"
      printf "  export PATH=\"%s:\$PATH\"\n" "$INSTALL_DIR"
      ;;
  esac
}

main
