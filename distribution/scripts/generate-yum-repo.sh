#!/bin/bash
# Generate YUM repository structure from .rpm packages
# Usage: ./generate-yum-repo.sh <rpm-dir> <output-dir> [gpg-key-id]
set -euo pipefail

RPM_DIR="${1:?Usage: $0 <rpm-dir> <output-dir> [gpg-key-id]}"
OUTPUT_DIR="${2:?Usage: $0 <rpm-dir> <output-dir> [gpg-key-id]}"
GPG_KEY_ID="${3:-}"

echo "==> Generating YUM repository"
echo "    RPMs: $RPM_DIR"
echo "    Output: $OUTPUT_DIR"

# Create architecture directories
mkdir -p "${OUTPUT_DIR}/x86_64"
mkdir -p "${OUTPUT_DIR}/aarch64"

# Copy RPMs to appropriate architecture directories
for rpm in "${RPM_DIR}"/*.rpm; do
    [ -f "$rpm" ] || continue
    name=$(basename "$rpm")
    case "$name" in
        *.x86_64.rpm)
            cp "$rpm" "${OUTPUT_DIR}/x86_64/"
            ;;
        *.aarch64.rpm)
            cp "$rpm" "${OUTPUT_DIR}/aarch64/"
            ;;
        *)
            echo "WARNING: Unknown architecture for $name, skipping"
            ;;
    esac
done

# Sign RPM packages
if [ -n "$GPG_KEY_ID" ]; then
    echo "==> Signing RPM packages with GPG key: ${GPG_KEY_ID}"
    for rpm in "${OUTPUT_DIR}"/*/*.rpm; do
        [ -f "$rpm" ] || continue
        echo "    Signing $(basename "$rpm")"
        rpm --addsign "$rpm" \
            --define "%_gpg_name ${GPG_KEY_ID}" \
            --define "%__gpg_sign_cmd %{__gpg} gpg --batch --no-verbose --no-armor --no-secmem-warning -u '%{_gpg_name}' -sbo %{__signature_filename} %{__plaintext_filename}" \
            2>/dev/null || echo "    WARNING: Could not sign $(basename "$rpm")"
    done
fi

# Generate repository metadata for each architecture
for arch in x86_64 aarch64; do
    arch_dir="${OUTPUT_DIR}/${arch}"
    if [ -n "$(ls -A "${arch_dir}"/*.rpm 2>/dev/null)" ]; then
        echo "==> Generating repodata for ${arch}"
        createrepo_c "${arch_dir}"
    else
        echo "==> No RPMs for ${arch}, skipping repodata"
    fi
done

echo "==> YUM repository generated at ${OUTPUT_DIR}"
