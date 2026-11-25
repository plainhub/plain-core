#!/usr/bin/env bash
# plainapp installer
set -euo pipefail

REPO="plainhub/plain-core"
BIN="plainapp"
SERVICE="plainapp"
INSTALL_DIR="/usr/local/bin"
RELEASE_PUBLIC_KEY='MCowBQYDK2VwAyEAwJ82lZQXMtwiMdQ67VxPVZXljZUAYDfudpvxAxBiImg='

fail() {
    echo "[plainapp] ERROR: $1" >&2
    exit 1
}

fetch() {
    curl --disable -fsSL --proto '=https' --proto-redir '=https' --connect-timeout 15 --max-time 600 --max-filesize "$2" "$1"
}

download() {
    fetch "$1" "$3" >"$2" || fail "Failed to download $1"
    [[ $(wc -c < "$2") -le "$3" ]] || fail "Release download exceeded its size limit"
}

case "$(uname -m)" in
    x86_64 | amd64) ARCH="amd64" ;;
    aarch64 | arm64) ARCH="arm64" ;;
    *) fail "Unsupported architecture: $(uname -m). Only amd64 and arm64 are supported." ;;
esac

command -v curl >/dev/null || fail "curl is required"
command -v openssl >/dev/null || fail "OpenSSL 3 is required for release signature verification"
command -v sha256sum >/dev/null || fail "sha256sum is required"
trusted_key=${PLAINAPP_RELEASE_PUBLIC_KEY:-$RELEASE_PUBLIC_KEY}

echo "[plainapp] Detecting platform... $ARCH"

[[ $# -le 1 ]] || fail "Usage: bash install.sh [release-tag]"
TAG=${1:-}
if [[ -z "$TAG" ]]; then
    TAG=$(
        fetch "https://api.github.com/repos/${REPO}/releases/latest" 1048576 |
            grep '"tag_name"' |
            sed 's/.*"tag_name": *"//' |
            sed 's/".*//'
    )
fi

if [[ ! "$TAG" =~ ^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$ ]]; then
    fail "Could not find a published stable release"
fi

echo "[plainapp] Latest version: $TAG"
ASSET="${BIN}-linux-${ARCH}.tar.gz"
BASE="https://github.com/${REPO}/releases/download/${TAG}"

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT
download "$BASE/release-manifest.txt" "$WORK/release-manifest.txt" 4096
download "$BASE/release-manifest.sig" "$WORK/release-manifest.sig" 64
printf '%s' "$trusted_key" | openssl base64 -d -A > "$WORK/release-key.der"
[[ $(wc -c < "$WORK/release-key.der") -eq 44 ]] &&
    [[ $(od -An -tx1 -N12 "$WORK/release-key.der" | tr -d ' \n') == 302a300506032b6570032100 ]] ||
    fail "Invalid release signing public key"
openssl pkeyutl -verify -pubin -keyform DER -inkey "$WORK/release-key.der" -rawin \
    -in "$WORK/release-manifest.txt" -sigfile "$WORK/release-manifest.sig" >/dev/null \
    || fail "Release manifest signature verification failed"
hash=$(awk -v version="${TAG#v}" -v archive="$ASSET" '
    NR == 1 { if ($0 != "version " version) exit 1 }
    NR == 2 { if ($1 != "schema" || length($2) != 64 || $2 ~ /[^0-9a-f]/ || NF != 2) exit 1 }
    NR >= 3 && NR <= 5 {
        name = (NR == 3 ? "plainapp-linux-amd64.tar.gz" : (NR == 4 ? "plainapp-linux-arm64.tar.gz" : "install.sh"))
        if (length($1) != 64 || $1 ~ /[^0-9a-f]/ || $0 != $1 "  " name) exit 1
        if (name == archive) hash = $1
    }
    END { if (NR != 5 || length(hash) != 64) exit 1; print hash }
' "$WORK/release-manifest.txt") || fail "Invalid release manifest"

echo "[plainapp] Downloading $ASSET ..."
download "$BASE/$ASSET" "$WORK/$ASSET" 134217728
(cd "$WORK" && printf '%s  %s\n' "$hash" "$ASSET" | sha256sum --check --status) || fail "Release archive checksum mismatch"
[[ $(tar -tzf "$WORK/$ASSET") == plainapp ]] || fail "Unexpected release archive contents"
tar -xOzf "$WORK/$ASSET" plainapp > "$WORK/plainapp" || fail "Failed to extract $ASSET"

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
