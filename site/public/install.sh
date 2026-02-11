#!/bin/sh
set -eu

REPO="example/irc"
INSTALL_DIR="${IRC_INSTALL_DIR:-/usr/local/bin}"
BINARIES="irc-server irc irc-gui"

main() {
    detect_platform
    get_latest_version
    echo "Installing irc ${VERSION} for ${OS}-${ARCH}..."

    tmpdir=$(mktemp -d)
    trap 'rm -rf "${tmpdir}"' EXIT

    for bin in ${BINARIES}; do
        download_binary "${bin}"
    done

    install_binaries
    echo ""
    echo "Installed to ${INSTALL_DIR}"
    echo "Run 'irc-server --help' to get started."
}

detect_platform() {
    OS=$(uname -s | tr '[:upper:]' '[:lower:]')
    case "${OS}" in
        linux)  OS="linux" ;;
        darwin) OS="darwin" ;;
        mingw*|msys*|cygwin*) OS="windows" ;;
        *)
            echo "Error: unsupported OS '${OS}'" >&2
            exit 1
            ;;
    esac

    ARCH=$(uname -m)
    case "${ARCH}" in
        x86_64|amd64) ARCH="x86_64" ;;
        aarch64|arm64) ARCH="aarch64" ;;
        *)
            echo "Error: unsupported architecture '${ARCH}'" >&2
            exit 1
            ;;
    esac

    EXT=""
    if [ "${OS}" = "windows" ]; then
        EXT=".exe"
    fi
}

get_latest_version() {
    VERSION=$(curl -sSf "https://api.github.com/repos/${REPO}/releases/latest" \
        | grep '"tag_name"' \
        | sed -E 's/.*"tag_name": *"([^"]+)".*/\1/')

    if [ -z "${VERSION}" ]; then
        echo "Error: could not determine latest version" >&2
        exit 1
    fi
}

download_binary() {
    bin="$1"
    asset="${bin}-${OS}-${ARCH}${EXT}"
    url="https://github.com/${REPO}/releases/latest/download/${asset}"
    dest="${tmpdir}/${bin}${EXT}"

    echo "Downloading ${asset}..."
    if ! curl -sSfL -o "${dest}" "${url}"; then
        echo "Error: failed to download ${url}" >&2
        exit 1
    fi
    chmod +x "${dest}"
}

install_binaries() {
    mkdir -p "${INSTALL_DIR}"

    need_sudo=""
    if [ ! -w "${INSTALL_DIR}" ]; then
        need_sudo="sudo"
        echo "Need sudo to install to ${INSTALL_DIR}"
    fi

    for bin in ${BINARIES}; do
        ${need_sudo} cp "${tmpdir}/${bin}${EXT}" "${INSTALL_DIR}/${bin}${EXT}"
    done
}

main
