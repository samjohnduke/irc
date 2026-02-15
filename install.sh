#!/bin/sh
# IRC Server & Client Installer
# Usage: curl -sSf https://raw.githubusercontent.com/samjohnduke/irc/main/install.sh | sh
#
# Environment variables:
#   IRC_INSTALL_DIR  - Installation directory (default: /usr/local/bin, or ~/.local/bin if not root)
#   IRC_VERSION      - Specific version to install (default: latest)
#   IRC_COMPONENTS   - Components to install: "all", "server", "cli" (default: all)

set -e

REPO="samjohnduke/irc"
GITHUB_API="https://api.github.com/repos/${REPO}/releases"
GITHUB_DOWNLOAD="https://github.com/${REPO}/releases/download"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

info() {
    printf "${CYAN}==>${NC} %s\n" "$1"
}

success() {
    printf "${GREEN}==>${NC} %s\n" "$1"
}

warn() {
    printf "${YELLOW}Warning:${NC} %s\n" "$1"
}

error() {
    printf "${RED}Error:${NC} %s\n" "$1" >&2
    exit 1
}

# Detect OS
detect_os() {
    case "$(uname -s)" in
        Linux*)  echo "linux" ;;
        Darwin*) echo "darwin" ;;
        MINGW*|MSYS*|CYGWIN*) echo "windows" ;;
        *) error "Unsupported operating system: $(uname -s)" ;;
    esac
}

# Detect architecture
detect_arch() {
    case "$(uname -m)" in
        x86_64|amd64) echo "x86_64" ;;
        aarch64|arm64) echo "aarch64" ;;
        *) error "Unsupported architecture: $(uname -m)" ;;
    esac
}

# Get latest release version
get_latest_version() {
    if command -v curl >/dev/null 2>&1; then
        curl -sSf "${GITHUB_API}/latest" | grep '"tag_name"' | sed -E 's/.*"tag_name": *"([^"]+)".*/\1/'
    elif command -v wget >/dev/null 2>&1; then
        wget -qO- "${GITHUB_API}/latest" | grep '"tag_name"' | sed -E 's/.*"tag_name": *"([^"]+)".*/\1/'
    else
        error "Neither curl nor wget found. Please install one of them."
    fi
}

# Download a file
download() {
    local url="$1"
    local dest="$2"

    if command -v curl >/dev/null 2>&1; then
        curl -sSfL "$url" -o "$dest"
    elif command -v wget >/dev/null 2>&1; then
        wget -qO "$dest" "$url"
    else
        error "Neither curl nor wget found."
    fi
}

# Main installation
main() {
    local os=$(detect_os)
    local arch=$(detect_arch)
    local suffix="${os}-${arch}"
    local ext=""

    if [ "$os" = "windows" ]; then
        ext=".exe"
    fi

    # Determine install directory
    local install_dir="${IRC_INSTALL_DIR:-}"
    if [ -z "$install_dir" ]; then
        if [ "$(id -u)" = "0" ]; then
            install_dir="/usr/local/bin"
        else
            install_dir="${HOME}/.local/bin"
        fi
    fi

    # Determine version
    local version="${IRC_VERSION:-}"
    if [ -z "$version" ]; then
        info "Fetching latest version..."
        version=$(get_latest_version)
        if [ -z "$version" ]; then
            error "Failed to get latest version. Please specify IRC_VERSION."
        fi
    fi

    # Determine components
    local components="${IRC_COMPONENTS:-all}"

    info "Installing IRC ${version} for ${os}/${arch}"
    info "Install directory: ${install_dir}"

    # Create install directory
    mkdir -p "$install_dir"

    # Create temp directory
    local tmp_dir=$(mktemp -d)
    trap "rm -rf '$tmp_dir'" EXIT

    # Download and install components
    if [ "$components" = "all" ] || [ "$components" = "server" ]; then
        local server_url="${GITHUB_DOWNLOAD}/${version}/irc-server-${suffix}${ext}"
        local server_dest="${install_dir}/irc-server${ext}"

        info "Downloading irc-server..."
        download "$server_url" "${tmp_dir}/irc-server${ext}"

        chmod +x "${tmp_dir}/irc-server${ext}"
        mv "${tmp_dir}/irc-server${ext}" "$server_dest"
        success "Installed irc-server to ${server_dest}"
    fi

    if [ "$components" = "all" ] || [ "$components" = "cli" ]; then
        local cli_url="${GITHUB_DOWNLOAD}/${version}/irc-${suffix}${ext}"
        local cli_dest="${install_dir}/irc${ext}"

        info "Downloading irc (CLI client)..."
        download "$cli_url" "${tmp_dir}/irc${ext}"

        chmod +x "${tmp_dir}/irc${ext}"
        mv "${tmp_dir}/irc${ext}" "$cli_dest"
        success "Installed irc to ${cli_dest}"
    fi

    # Check if install dir is in PATH
    case ":$PATH:" in
        *":${install_dir}:"*) ;;
        *)
            warn "${install_dir} is not in your PATH."
            echo ""
            echo "Add it to your shell profile:"
            echo "  export PATH=\"${install_dir}:\$PATH\""
            echo ""
            ;;
    esac

    echo ""
    success "Installation complete!"
    echo ""
    echo "Get started:"
    echo "  irc-server --help    # Run the IRC server"
    echo "  irc --help           # Run the IRC client"
    echo ""
}

main "$@"
