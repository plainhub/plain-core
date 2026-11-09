#!/usr/bin/env bash
# plainapp installer
# Usage:  curl -fsSL https://raw.githubusercontent.com/plainhub/plain-core/main/install.sh | bash
set -euo pipefail

REPO="plainhub/plain-core"
BIN="plainapp"
SERVICE="plainapp"
INSTALL_DIR="/usr/local/bin"

fail() {
    echo "[plainapp] ERROR: $1" >&2
    exit 1
}

fetch() {
    if command -v curl &>/dev/null; then
        curl -fsSL "$1"
    elif command -v wget &>/dev/null; then
        wget -qO- "$1"
    else
        fail "curl or wget is required"
    fi
}

download() {
    fetch "$1" >"$2" || fail "Failed to download $1"
}

case "$(uname -m)" in
    x86_64 | amd64) ARCH="amd64" ;;
    aarch64 | arm64) ARCH="arm64" ;;
    *) fail "Unsupported architecture: $(uname -m). Only amd64 and arm64 are supported." ;;
esac

echo "[plainapp] Detecting platform... $ARCH"

TAG=$(
    fetch "https://api.github.com/repos/${REPO}/releases/latest" |
        grep '"tag_name"' |
        sed 's/.*"tag_name": *"//' |
        sed 's/".*//'
)

if [[ -z "$TAG" ]]; then
    fail "Could not find a release. Is there a published release?"
fi

echo "[plainapp] Latest version: $TAG"
ASSET="${BIN}-linux-${ARCH}.tar.gz"
URL="https://github.com/${REPO}/releases/download/${TAG}/${ASSET}"
echo "[plainapp] Downloading $ASSET ..."

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT
download "$URL" "$WORK/$ASSET"
tar -xzf "$WORK/$ASSET" -C "$WORK" || fail "Failed to extract $ASSET."

sudo install -m 0755 "$WORK/$BIN" "$INSTALL_DIR/$BIN"
echo "[plainapp] Installed $BIN to $INSTALL_DIR."

sudo "$INSTALL_DIR/$BIN" install
sudo systemctl enable --now "$SERVICE"

IP=$(hostname -I 2>/dev/null | awk '{for (i=1;i<=NF;i++) if ($i!~/^127\./ && $i~/\./) {print $i; exit}}')
IP=${IP:-localhost}

echo ""
echo "[plainapp] Installation complete."
echo "[plainapp] Access the web UI:"
echo "  http://$IP:8080"
echo "  https://$IP:8443  (self-signed certificate)"
echo "[plainapp] Sign in with the admin password configured during installation."
echo "[plainapp] To change it later, stop the service and run: sudo $BIN passwd"
