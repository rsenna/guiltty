#!/usr/bin/env bash
# fetch-kitty.sh — download a pinned, signature-verified kitty binary.
#
# Usage: scripts/fetch-kitty.sh [TARGET_DIR]
#   TARGET_DIR defaults to ./target/kitty
#
# Downloads kitty v0.49.2 for the current platform (Linux x86_64/arm64,
# macOS), verifies the download against Kovid Goyal's published GPG
# signature (pinned to his full fingerprint, not just the key ID), and
# extracts a runnable `kitty` binary into TARGET_DIR.
#
# BSD is out of scope for this automated path: kitty publishes only the
# companion `kitten` CLI for BSDs, not the full terminal, so BSD users should
# install via the platform package manager (e.g. FreeBSD `pkg install kitty`).
#
# kitty is dev-only tooling: it is consumed purely as an external subprocess
# binary and never appears in any Cargo.toml.

set -euo pipefail

KITTY_VERSION="0.49.2"
# Kovid Goyal's GPG key: the full fingerprint is the authority the signature is
# checked against (the short key ID alone is not enough — see the verify step).
GPG_KEY_ID="06BC317B515ACE7C"
GPG_FULL_FINGERPRINT="3CE1780F78DD88DF45194FD706BC317B515ACE7C"
GPG_KEY_URL="https://github.com/kovidgoyal.gpg"
BASE_URL="https://github.com/kovidgoyal/kitty/releases/download/v${KITTY_VERSION}"

TARGET_DIR="${1:-./target/kitty}"

# --- prerequisites --------------------------------------------------------
if ! command -v gpg >/dev/null 2>&1; then
    echo "Error: GnuPG (gpg) is required to verify the kitty download, but it was not found on PATH." >&2
    echo "Install it first (macOS: 'brew install gnupg'; Debian/Ubuntu: 'apt install gnupg')." >&2
    exit 1
fi

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
# Detach any mounted DMG (Darwin) and remove the temp dir on exit.
DETACH=""
trap '[ -n "$DETACH" ] && hdiutil detach "$DETACH" >/dev/null 2>&1 || true; rm -rf "$WORK"' EXIT

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
# gpg --verify alone trusts ANY key in the keyring that validates the signature.
# Pin the signer to Kovid's full fingerprint so a substituted key (or a
# compromised keyring) cannot silently pass. VALIDSIG's first field is the full
# fingerprint of the key that actually made the signature.
SIGNER="$(gpg --status-fd 1 --verify "$WORK/$ASSET.sig" "$WORK/$ASSET" 2>/dev/null \
    | grep -o 'VALIDSIG [0-9A-F]\{16,\}' | head -1 | awk '{print $2}')"
if [ "$SIGNER" != "$GPG_FULL_FINGERPRINT" ]; then
    echo "Error: signature was not made by the pinned kitty signing key (${GPG_FULL_FINGERPRINT}); got '${SIGNER:-none}'" >&2
    exit 1
fi

# --- extract --------------------------------------------------------------
mkdir -p "$TARGET_DIR"
case "$OS" in
    Linux)
        tar -xJf "$WORK/$ASSET" -C "$WORK"
        # The txz extracts to bin/kitty plus lib/ and share/. A partial copy must
        # fail loudly rather than report success, so no `|| true` here.
        cp -R "$WORK/bin" "$WORK/lib" "$WORK/share" "$TARGET_DIR/"
        test -x "$TARGET_DIR/bin/kitty" || {
            echo "Error: kitty binary not found after extraction" >&2
            exit 1
        }
        ;;
    Darwin)
        MOUNT="$(hdiutil attach "$WORK/$ASSET" -nobrowse -readonly | tail -1 | awk '{print $NF}')"
        DETACH="$MOUNT"
        cp -R "$MOUNT/kitty.app" "$TARGET_DIR/"
        hdiutil detach "$MOUNT" >/dev/null
        DETACH=""
        test -x "$TARGET_DIR/kitty.app/Contents/MacOS/kitty" || {
            echo "Error: kitty binary not found after extraction" >&2
            exit 1
        }
        ;;
esac

echo "kitty $KITTY_VERSION installed to $TARGET_DIR"