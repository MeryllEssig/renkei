#!/bin/sh
set -eu

REPO="meryll/renkei"
INSTALL_DIR="${RK_INSTALL_DIR:-$HOME/.local/bin}"
BINARY="rk"

main() {
    os="$(detect_os)"
    arch="$(detect_arch)"
    artifact="$(artifact_name "$os" "$arch")"

    if [ -z "$artifact" ]; then
        err "Unsupported platform: ${os}/${arch}"
    fi

    version="$(latest_version)"
    url="https://github.com/${REPO}/releases/download/${version}/${artifact}"

    info "Installing rk ${version} (${os}/${arch})"

    tmpdir="$(mktemp -d)"
    trap 'rm -rf "$tmpdir"' EXIT

    info "Downloading ${url}"
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL -o "${tmpdir}/${artifact}" "$url"
    elif command -v wget >/dev/null 2>&1; then
        wget -qO "${tmpdir}/${artifact}" "$url"
    else
        err "Neither curl nor wget found. Install one and retry."
    fi

    mkdir -p "$INSTALL_DIR"
    mv "${tmpdir}/${artifact}" "${INSTALL_DIR}/${BINARY}"
    chmod +x "${INSTALL_DIR}/${BINARY}"

    info "Installed to ${INSTALL_DIR}/${BINARY}"

    if ! echo "$PATH" | tr ':' '\n' | grep -qx "$INSTALL_DIR"; then
        warn "${INSTALL_DIR} is not in your PATH. Add it:"
        warn "  export PATH=\"${INSTALL_DIR}:\$PATH\""
    fi

    info "Done. Run 'rk --help' to get started."
}

detect_os() {
    case "$(uname -s)" in
        Linux*)  echo "linux" ;;
        Darwin*) echo "darwin" ;;
        MINGW*|MSYS*|CYGWIN*) echo "windows" ;;
        *) echo "unknown" ;;
    esac
}

detect_arch() {
    case "$(uname -m)" in
        x86_64|amd64) echo "x86_64" ;;
        arm64|aarch64) echo "aarch64" ;;
        *) echo "unknown" ;;
    esac
}

artifact_name() {
    _os="$1"
    _arch="$2"
    case "${_os}-${_arch}" in
        linux-x86_64)   echo "rk-linux-x86_64" ;;
        linux-aarch64)  echo "rk-linux-aarch64" ;;
        darwin-x86_64)  echo "rk-darwin-x86_64" ;;
        darwin-aarch64) echo "rk-darwin-aarch64" ;;
        windows-x86_64) echo "rk-windows-x86_64.exe" ;;
        *) echo "" ;;
    esac
}

latest_version() {
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL -o /dev/null -w '%{url_effective}' "https://github.com/${REPO}/releases/latest" 2>/dev/null \
            | rev | cut -d'/' -f1 | rev
    elif command -v wget >/dev/null 2>&1; then
        wget --spider --max-redirect=0 "https://github.com/${REPO}/releases/latest" 2>&1 \
            | grep -i 'Location:' | sed 's|.*/||' | tr -d '\r'
    else
        err "Neither curl nor wget found."
    fi
}

info() { printf '\033[0;32m%s\033[0m\n' "$1"; }
warn() { printf '\033[0;33m%s\033[0m\n' "$1"; }
err()  { printf '\033[0;31mError: %s\033[0m\n' "$1" >&2; exit 1; }

main
