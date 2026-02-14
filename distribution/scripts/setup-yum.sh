#!/bin/sh
# Add Velos YUM repository (RHEL/CentOS/Fedora)
# Usage: curl -fsSL https://releases.velos.dev/setup-yum.sh | sudo bash
set -e

REPO_URL="${VELOS_REPO_URL:-https://releases.velos.dev}"

echo "Adding Velos YUM repository..."

# Import GPG key
echo "Importing GPG key..."
rpm --import "${REPO_URL}/gpg.key"

# Add repository
cat > /etc/yum.repos.d/velos.repo <<EOF
[velos]
name=Velos - High-performance AI-friendly process manager
baseurl=${REPO_URL}/rpm/\$basearch
enabled=1
gpgcheck=1
gpgkey=${REPO_URL}/gpg.key
EOF

# Install
echo "Installing Velos..."
if command -v dnf >/dev/null 2>&1; then
    dnf install -y velos
else
    yum install -y velos
fi

echo ""
echo "Velos installed successfully!"
velos --version
