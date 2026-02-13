#!/bin/sh
# Velos installer — high-performance AI-friendly process manager
# https://github.com/Dave93/velos
#
# Usage:
#   curl -fsSL https://releases.velos.dev/install.sh | bash
#   curl -fsSL https://releases.velos.dev/install.sh | bash -s v0.1.1
#
# Environment variables:
#   VELOS_INSTALL     — install directory (default: $HOME/.velos)
#   VELOS_CDN_URL     — CDN base URL (default: auto-detect)
#   VELOS_VERSION     — version to install (default: latest)
#   VELOS_NO_MODIFY_PATH — if set, don't modify shell config

set -e

# ── Colors (only if TTY) ──────────────────────────────────────────

if [ -t 1 ] && [ -t 2 ]; then
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[0;33m'
    BLUE='\033[0;34m'
    BOLD='\033[1m'
    DIM='\033[2m'
    RESET='\033[0m'
else
    RED='' GREEN='' YELLOW='' BLUE='' BOLD='' DIM='' RESET=''
fi

# ── Helpers ───────────────────────────────────────────────────────

info() {
    printf "${BLUE}info${RESET} %s\n" "$1"
}

warn() {
    printf "${YELLOW}warn${RESET} %s\n" "$1" >&2
}

error() {
    printf "${RED}error${RESET} %s\n" "$1" >&2
    exit 1
}

success() {
    printf "${GREEN}success${RESET} %s\n" "$1"
}

command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# ── Platform detection ────────────────────────────────────────────

detect_platform() {
    PLATFORM=$(uname -ms)

    case "$PLATFORM" in
        'Darwin x86_64')    TARGET="macos-x86_64" ;;
        'Darwin arm64')     TARGET="macos-arm64" ;;
        'Linux x86_64')     TARGET="linux-x86_64" ;;
        'Linux aarch64')    TARGET="linux-arm64" ;;
        'Linux arm64')      TARGET="linux-arm64" ;;
        *)
            error "Unsupported platform: $PLATFORM. Velos supports macOS and Linux on x86_64 and arm64."
            ;;
    esac

    # Check for musl (Alpine Linux, etc.)
    if [ "$(uname -s)" = "Linux" ]; then
        case "$(cat /proc/version 2>/dev/null || true)" in
            *Alpine*|*musl*)
                warn "Detected musl libc (Alpine Linux). Velos currently requires glibc."
                warn "Using glibc build — may work with musl compatibility layer."
                ;;
        esac
    fi
}

# ── Version detection ─────────────────────────────────────────────

detect_version() {
    # Priority: argument > env var > latest from GitHub
    if [ -n "$1" ]; then
        VERSION="$1"
    elif [ -n "$VELOS_VERSION" ]; then
        VERSION="$VELOS_VERSION"
    else
        VERSION=$(get_latest_version)
    fi

    # Strip leading 'v' if present
    VERSION="${VERSION#v}"
    info "Installing Velos v${VERSION} for ${TARGET}"
}

get_latest_version() {
    if command_exists curl; then
        curl -fsSL "https://api.github.com/repos/Dave93/velos/releases/latest" 2>/dev/null |
            sed -n 's/.*"tag_name": *"v\{0,1\}\([^"]*\)".*/\1/p' | head -1
    elif command_exists wget; then
        wget -qO- "https://api.github.com/repos/Dave93/velos/releases/latest" 2>/dev/null |
            sed -n 's/.*"tag_name": *"v\{0,1\}\([^"]*\)".*/\1/p' | head -1
    fi

    if [ -z "$VERSION" ]; then
        # Fallback to hardcoded version
        echo "0.1.0"
    fi
}

# ── Download ──────────────────────────────────────────────────────

GITHUB_REPO="Dave93/velos"
ARCHIVE_NAME="velos-${TARGET}.tar.gz"

get_download_urls() {
    CDN_URL="${VELOS_CDN_URL:-}"
    GITHUB_URL="https://github.com/${GITHUB_REPO}/releases/download/v${VERSION}"

    if [ -n "$CDN_URL" ]; then
        PRIMARY_URL="${CDN_URL}/v${VERSION}/${ARCHIVE_NAME}"
        FALLBACK_URL="${GITHUB_URL}/${ARCHIVE_NAME}"
        CHECKSUM_PRIMARY="${CDN_URL}/v${VERSION}/checksums.txt"
        CHECKSUM_FALLBACK="${GITHUB_URL}/checksums.txt"
    else
        PRIMARY_URL="${GITHUB_URL}/${ARCHIVE_NAME}"
        FALLBACK_URL=""
        CHECKSUM_PRIMARY="${GITHUB_URL}/checksums.txt"
        CHECKSUM_FALLBACK=""
    fi
}

download() {
    url="$1"
    output="$2"

    if command_exists curl; then
        curl -fsSL --progress-bar "$url" -o "$output" 2>&1
    elif command_exists wget; then
        wget -q --show-progress "$url" -O "$output" 2>&1
    else
        error "Neither curl nor wget found. Please install one of them."
    fi
}

download_with_fallback() {
    url="$1"
    fallback="$2"
    output="$3"
    label="$4"

    info "Downloading ${label}..."

    if download "$url" "$output" 2>/dev/null; then
        return 0
    fi

    if [ -n "$fallback" ]; then
        warn "CDN download failed, falling back to GitHub Releases..."
        if download "$fallback" "$output" 2>/dev/null; then
            return 0
        fi
    fi

    error "Failed to download ${label}. Check your internet connection and try again."
}

# ── Checksum verification ─────────────────────────────────────────

verify_checksum() {
    archive_path="$1"
    checksums_path="$2"

    if [ ! -f "$checksums_path" ]; then
        warn "Checksum file not found, skipping verification."
        return 0
    fi

    # Extract expected hash for our archive
    expected=$(grep "${ARCHIVE_NAME}" "$checksums_path" | awk '{print $1}' | head -1)
    if [ -z "$expected" ]; then
        warn "No checksum found for ${ARCHIVE_NAME}, skipping verification."
        return 0
    fi

    # Calculate actual hash
    if command_exists sha256sum; then
        actual=$(sha256sum "$archive_path" | awk '{print $1}')
    elif command_exists shasum; then
        actual=$(shasum -a 256 "$archive_path" | awk '{print $1}')
    else
        warn "No sha256sum or shasum available, skipping checksum verification."
        return 0
    fi

    if [ "$expected" != "$actual" ]; then
        error "Checksum verification failed!
  Expected: ${expected}
  Actual:   ${actual}
This could indicate a corrupted download or a supply chain attack."
    fi

    info "Checksum verified ${DIM}(SHA-256)${RESET}"
}

# ── Install ───────────────────────────────────────────────────────

install_binary() {
    INSTALL_DIR="${VELOS_INSTALL:-$HOME/.velos}"
    BIN_DIR="${INSTALL_DIR}/bin"

    # Create install directory
    mkdir -p "$BIN_DIR"

    # Extract archive
    info "Extracting archive..."
    tar xzf "${TMP_DIR}/${ARCHIVE_NAME}" -C "$TMP_DIR"

    # Find the binary (it's inside velos-{target}/ directory)
    if [ -f "${TMP_DIR}/velos-${TARGET}/velos" ]; then
        BINARY_PATH="${TMP_DIR}/velos-${TARGET}/velos"
    elif [ -f "${TMP_DIR}/velos" ]; then
        BINARY_PATH="${TMP_DIR}/velos"
    else
        error "Could not find velos binary in archive."
    fi

    # Install binary
    chmod +x "$BINARY_PATH"
    mv "$BINARY_PATH" "${BIN_DIR}/velos"

    info "Installed to ${BOLD}${BIN_DIR}/velos${RESET}"
}

# ── Shell config ──────────────────────────────────────────────────

update_shell_config() {
    if [ -n "$VELOS_NO_MODIFY_PATH" ]; then
        return 0
    fi

    BIN_DIR="${INSTALL_DIR}/bin"
    EXPORT_LINE="export VELOS_INSTALL=\"${INSTALL_DIR}\""
    PATH_LINE="export PATH=\"${BIN_DIR}:\$PATH\""

    # Check if already in PATH
    case ":$PATH:" in
        *":${BIN_DIR}:"*)
            return 0
            ;;
    esac

    SHELLS_UPDATED=""

    # Bash
    for rc in "$HOME/.bashrc" "$HOME/.bash_profile"; do
        if [ -f "$rc" ]; then
            if ! grep -q "VELOS_INSTALL" "$rc" 2>/dev/null; then
                printf '\n# Velos\n%s\n%s\n' "$EXPORT_LINE" "$PATH_LINE" >> "$rc"
                SHELLS_UPDATED="${SHELLS_UPDATED} bash"
            fi
            break
        fi
    done

    # Zsh
    ZSHRC="$HOME/.zshrc"
    if [ -f "$ZSHRC" ]; then
        if ! grep -q "VELOS_INSTALL" "$ZSHRC" 2>/dev/null; then
            printf '\n# Velos\n%s\n%s\n' "$EXPORT_LINE" "$PATH_LINE" >> "$ZSHRC"
            SHELLS_UPDATED="${SHELLS_UPDATED} zsh"
        fi
    fi

    # Fish
    FISH_CONFIG="$HOME/.config/fish/conf.d/velos.fish"
    if [ -d "$HOME/.config/fish" ]; then
        mkdir -p "$HOME/.config/fish/conf.d"
        if [ ! -f "$FISH_CONFIG" ] || ! grep -q "VELOS_INSTALL" "$FISH_CONFIG" 2>/dev/null; then
            cat > "$FISH_CONFIG" <<FISH_EOF
# Velos
set -gx VELOS_INSTALL "$INSTALL_DIR"
fish_add_path "$BIN_DIR"
FISH_EOF
            SHELLS_UPDATED="${SHELLS_UPDATED} fish"
        fi
    fi

    if [ -n "$SHELLS_UPDATED" ]; then
        info "Updated shell config for:${SHELLS_UPDATED}"
    fi
}

# ── Main ──────────────────────────────────────────────────────────

main() {
    printf "\n"
    printf "  %bVelos%b installer\n" "$BOLD" "$RESET"
    printf "  %bHigh-performance AI-friendly process manager%b\n" "$DIM" "$RESET"
    printf "\n"

    # Detect platform
    detect_platform

    # Detect version (first argument = version)
    detect_version "$1"

    # Build download URLs
    ARCHIVE_NAME="velos-${TARGET}.tar.gz"
    get_download_urls

    # Create temp directory
    TMP_DIR=$(mktemp -d)
    trap 'rm -rf "$TMP_DIR"' EXIT

    # Download checksum file (best effort)
    download "$CHECKSUM_PRIMARY" "${TMP_DIR}/checksums.txt" 2>/dev/null ||
        { [ -n "$CHECKSUM_FALLBACK" ] && download "$CHECKSUM_FALLBACK" "${TMP_DIR}/checksums.txt" 2>/dev/null; } ||
        true

    # Download archive
    download_with_fallback "$PRIMARY_URL" "$FALLBACK_URL" "${TMP_DIR}/${ARCHIVE_NAME}" "velos v${VERSION}"

    # Verify checksum
    verify_checksum "${TMP_DIR}/${ARCHIVE_NAME}" "${TMP_DIR}/checksums.txt"

    # Install
    install_binary

    # Update shell config
    update_shell_config

    # Verify installation
    printf "\n"
    if "${BIN_DIR}/velos" --version >/dev/null 2>&1; then
        INSTALLED_VERSION=$("${BIN_DIR}/velos" --version 2>&1 | head -1)
        success "Velos installed! ${DIM}(${INSTALLED_VERSION})${RESET}"
    else
        success "Velos v${VERSION} installed to ${BIN_DIR}/velos"
    fi

    # Print next steps
    printf "\n"
    case ":$PATH:" in
        *":${BIN_DIR}:"*)
            ;;
        *)
            printf "  %bTo get started, restart your shell or run:%b\n" "$YELLOW" "$RESET"
            printf "\n"
            printf "    export PATH=\"%s:\$PATH\"\n" "$BIN_DIR"
            printf "\n"
            ;;
    esac
    printf "  %b# Start the daemon%b\n" "$DIM" "$RESET"
    printf "  velos daemon &\n"
    printf "\n"
    printf "  %b# Start a process%b\n" "$DIM" "$RESET"
    printf "  velos start app.js --name my-app\n"
    printf "\n"
    printf "  %b# See all commands%b\n" "$DIM" "$RESET"
    printf "  velos --help\n"
    printf "\n"
}

main "$@"
