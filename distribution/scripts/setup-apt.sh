#!/bin/sh
# Add Velos APT repository (Debian/Ubuntu)
# Usage: curl -fsSL https://releases.velos.dev/setup-apt.sh | sudo bash
set -e

REPO_URL="${VELOS_REPO_URL:-https://releases.velos.dev}"

echo "Adding Velos APT repository..."

# Install prerequisites
apt-get update -qq
apt-get install -y -qq curl gpg >/dev/null 2>&1

# Download and install GPG key
echo "Downloading GPG key..."
curl -fsSL "${REPO_URL}/gpg.key" | gpg --dearmor -o /usr/share/keyrings/velos-archive-keyring.gpg

# Add repository
echo "deb [signed-by=/usr/share/keyrings/velos-archive-keyring.gpg] ${REPO_URL}/apt stable main" \
    > /etc/apt/sources.list.d/velos.list

# Update and install
echo "Installing Velos..."
apt-get update -qq
apt-get install -y velos

echo ""
echo "Velos installed successfully!"
velos --version
