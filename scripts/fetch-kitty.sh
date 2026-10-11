#!/usr/bin/env bash
# fetch-kitty.sh — download a pinned, signature-verified kitty binary.
#
# Usage: scripts/fetch-kitty.sh [TARGET_DIR]
#   TARGET_DIR defaults to ./target/kitty
#
# Downloads kitty v0.49.2 for the current platform (Linux x86_64/arm64,
# macOS), verifies the download against Kovid Goyal's published GPG
# signature, and extracts a runnable `kitty` binary into TARGET_DIR.
#
# BSD is out of scope for this automated path: kitty publishes only the
# companion `kitten` CLI for BSDs, not the full terminal, so BSD users should
# install via the platform package manager (e.g. FreeBSD `pkg install kitty`).
#
# kitty is dev-only tooling: it is consumed purely as an external subprocess
# binary and never appears in any Cargo.toml.

set -euo pipefail

KITTY_VERSION="0.49.2"
# Kovid Goyal's GPG key (full fingerprint 3CE1780F78DD88DF45194FD706BC317B515ACE7C).
GPG_KEY_ID="06BC317B515ACE7C"
GPG_KEY_URL="https://github.com/kovidgoyal.gpg"
BASE_URL="https://github.com/kovidgoyal/kitty/releases/download/v${KITTY_VERSION}"

TARGET_DIR="${1:-./target/kitty}"

# --- platform detection ---------------------------------------------------
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
    Linux)
        case "$ARCH" in
            x86_64) ASSET="kitty-${KITTY_VERSION}-x86_64.txz" ;;
            aarch64 | arm64) ASSET="kitty-${KITTY_VERSION}-arm64.txz" ;;
            *)
                echo "Error: unsupported Linux architecture '$ARCH'" >&2
                exit 1
                ;;
        esac
        ;;
    Darwin)
        ASSET="kitty-${KITTY_VERSION}.dmg"
        ;;
    *)
        echo "Error: unsupported OS '$OS'. BSD users: install kitty via the platform package manager (e.g. 'pkg install kitty')." >&2
        exit 1
        ;;
esac

# --- download -------------------------------------------------------------
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

echo "Downloading $ASSET ..."
curl -fsSL -o "$WORK/$ASSET" "$BASE_URL/$ASSET"
curl -fsSL -o "$WORK/$ASSET.sig" "$BASE_URL/$ASSET.sig"

# --- verify signature -----------------------------------------------------
# Import Kovid Goyal's key if not already present (fetched from GitHub, which
# is more reliable than a keyserver for this key).
if ! gpg --list-keys "$GPG_KEY_ID" >/dev/null 2>&1; then
    echo "Importing Kovid Goyal's GPG key ..."
    curl -fsSL "$GPG_KEY_URL" | gpg --import
fi

echo "Verifying signature ..."
gpg --verify "$WORK/$ASSET.sig" "$WORK/$ASSET"

# --- extract --------------------------------------------------------------
mkdir -p "$TARGET_DIR"
case "$OS" in
    Linux)
        tar -xJf "$WORK/$ASSET" -C "$WORK"
        # The txz extracts to bin/kitty plus lib/ and share/.
        cp -R "$WORK/bin" "$WORK/lib" "$WORK/share" "$TARGET_DIR/" 2>/dev/null || true
        test -x "$TARGET_DIR/bin/kitty" || {
            echo "Error: kitty binary not found after extraction" >&2
            exit 1
        }
        ;;
    Darwin)
        MOUNT="$(hdiutil attach "$WORK/$ASSET" -nobrowse -readonly | tail -1 | awk '{print $NF}')"
        cp -R "$MOUNT/kitty.app" "$TARGET_DIR/"
        hdiutil detach "$MOUNT" >/dev/null
        test -x "$TARGET_DIR/kitty.app/Contents/MacOS/kitty" || {
            echo "Error: kitty binary not found after extraction" >&2
            exit 1
        }
        ;;
esac

echo "kitty $KITTY_VERSION installed to $TARGET_DIR"
