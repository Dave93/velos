#!/bin/bash
# Generate APT repository structure from .deb packages
# Usage: ./generate-apt-repo.sh <deb-dir> <output-dir> [gpg-key-id]
set -euo pipefail

DEB_DIR="${1:?Usage: $0 <deb-dir> <output-dir> [gpg-key-id]}"
OUTPUT_DIR="${2:?Usage: $0 <deb-dir> <output-dir> [gpg-key-id]}"
GPG_KEY_ID="${3:-}"

CODENAME="stable"
COMPONENT="main"

echo "==> Generating APT repository"
echo "    Debs: $DEB_DIR"
echo "    Output: $OUTPUT_DIR"

# Create directory structure
for arch in amd64 arm64; do
    mkdir -p "${OUTPUT_DIR}/dists/${CODENAME}/${COMPONENT}/binary-${arch}"
    mkdir -p "${OUTPUT_DIR}/pool/${COMPONENT}/v/velos"
done

# Copy .deb files to pool
cp "${DEB_DIR}"/*.deb "${OUTPUT_DIR}/pool/${COMPONENT}/v/velos/" 2>/dev/null || true

# Generate Packages files for each architecture
for arch in amd64 arm64; do
    echo "==> Generating Packages for ${arch}"
    BINARY_DIR="${OUTPUT_DIR}/dists/${CODENAME}/${COMPONENT}/binary-${arch}"

    cd "${OUTPUT_DIR}"
    dpkg-scanpackages --arch "${arch}" "pool/${COMPONENT}" > "${BINARY_DIR}/Packages"
    gzip -9c "${BINARY_DIR}/Packages" > "${BINARY_DIR}/Packages.gz"
    cd - >/dev/null
done

# Generate Release file
echo "==> Generating Release file"
RELEASE_FILE="${OUTPUT_DIR}/dists/${CODENAME}/Release"

{
    echo "Origin: Velos"
    echo "Label: Velos"
    echo "Suite: ${CODENAME}"
    echo "Codename: ${CODENAME}"
    echo "Components: ${COMPONENT}"
    echo "Architectures: amd64 arm64"
    echo "Date: $(date -Ru)"
    echo "Description: Velos package repository"
    echo "MD5Sum:"
} > "${RELEASE_FILE}"

# Calculate checksums for all Packages files
cd "${OUTPUT_DIR}/dists/${CODENAME}"
for file in $(find "${COMPONENT}" -name 'Packages*' -type f | sort); do
    size=$(wc -c < "$file" | tr -d ' ')
    md5=$(md5sum "$file" | awk '{print $1}')
    printf " %s %16d %s\n" "$md5" "$size" "$file" >> Release
done

echo "SHA256:" >> Release
for file in $(find "${COMPONENT}" -name 'Packages*' -type f | sort); do
    size=$(wc -c < "$file" | tr -d ' ')
    sha256=$(sha256sum "$file" | awk '{print $1}')
    printf " %s %16d %s\n" "$sha256" "$size" "$file" >> Release
done
cd - >/dev/null

# GPG sign Release file
if [ -n "$GPG_KEY_ID" ]; then
    echo "==> Signing Release with GPG key: ${GPG_KEY_ID}"

    # Detached signature
    gpg --default-key "${GPG_KEY_ID}" \
        --armor --detach-sign \
        --output "${OUTPUT_DIR}/dists/${CODENAME}/Release.gpg" \
        "${RELEASE_FILE}"

    # Inline signature (InRelease)
    gpg --default-key "${GPG_KEY_ID}" \
        --armor --clearsign \
        --output "${OUTPUT_DIR}/dists/${CODENAME}/InRelease" \
        "${RELEASE_FILE}"

    echo "==> Signed: Release.gpg + InRelease"
else
    echo "==> WARNING: No GPG key provided, skipping signing"
fi

echo "==> APT repository generated at ${OUTPUT_DIR}"
