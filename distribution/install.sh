#!/bin/sh
# Velos installer — downloads the correct binary for your platform
# Usage: curl -fsSL https://raw.githubusercontent.com/user/velos/main/distribution/install.sh | sh
set -e

VERSION="${VELOS_VERSION:-0.1.0}"
REPO="user/velos"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"

# Detect OS
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
case "$OS" in
  linux)  OS="linux" ;;
  darwin) OS="macos" ;;
  *)
    echo "Error: Unsupported operating system: $OS"
    exit 1
    ;;
esac

# Detect architecture
ARCH=$(uname -m)
case "$ARCH" in
  x86_64|amd64)    ARCH="x86_64" ;;
  aarch64|arm64)   ARCH="arm64" ;;
  *)
    echo "Error: Unsupported architecture: $ARCH"
    exit 1
    ;;
esac

BINARY="velos-${OS}-${ARCH}"
URL="https://github.com/${REPO}/releases/download/v${VERSION}/${BINARY}"
CHECKSUM_URL="${URL}.sha256"

echo "Installing Velos v${VERSION} for ${OS}/${ARCH}..."
echo "  Binary: ${BINARY}"
echo "  Target: ${INSTALL_DIR}/velos"
echo ""

# Download binary
TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

echo "Downloading ${URL}..."
curl -fsSL "$URL" -o "${TMP_DIR}/velos"

# Verify checksum if sha256sum is available
echo "Downloading checksum..."
if curl -fsSL "$CHECKSUM_URL" -o "${TMP_DIR}/velos.sha256" 2>/dev/null; then
  cd "$TMP_DIR"
  if command -v sha256sum >/dev/null 2>&1; then
    echo "$(<velos.sha256)" | sed "s/${BINARY}/velos/" | sha256sum -c - || {
      echo "Error: Checksum verification failed!"
      exit 1
    }
  elif command -v shasum >/dev/null 2>&1; then
    EXPECTED=$(awk '{print $1}' velos.sha256)
    ACTUAL=$(shasum -a 256 velos | awk '{print $1}')
    if [ "$EXPECTED" != "$ACTUAL" ]; then
      echo "Error: Checksum verification failed!"
      echo "  Expected: $EXPECTED"
      echo "  Actual:   $ACTUAL"
      exit 1
    fi
    echo "Checksum verified."
  else
    echo "Warning: No sha256sum or shasum available, skipping checksum verification."
  fi
  cd - >/dev/null
else
  echo "Warning: Could not download checksum file, skipping verification."
fi

# Install
chmod +x "${TMP_DIR}/velos"
if [ -w "$INSTALL_DIR" ]; then
  mv "${TMP_DIR}/velos" "${INSTALL_DIR}/velos"
else
  echo "Need sudo to install to ${INSTALL_DIR}"
  sudo mv "${TMP_DIR}/velos" "${INSTALL_DIR}/velos"
fi

echo ""
echo "Velos v${VERSION} installed successfully!"
"${INSTALL_DIR}/velos" --version
