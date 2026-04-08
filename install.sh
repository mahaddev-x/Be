#!/usr/bin/env bash
# BeeHive installer for macOS and Linux
# Usage: curl -fsSL https://mahaddev-x.github.io/beehive/install.sh | bash

set -euo pipefail

REPO="mahaddev-x/beehive"
BIN_DIR="$HOME/.beehive/bin"

# ── Detect platform ────────────────────────────────────────────────────────────
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$OS" in
  darwin) OS_KEY="darwin" ;;
  linux)  OS_KEY="linux"  ;;
  *) echo "Unsupported OS: $OS" && exit 1 ;;
esac

case "$ARCH" in
  x86_64|amd64)   ARCH_KEY="x64"   ;;
  aarch64|arm64)  ARCH_KEY="arm64" ;;
  *) echo "Unsupported architecture: $ARCH" && exit 1 ;;
esac

PLATFORM="bun-$OS_KEY-$ARCH_KEY"

echo ""
echo "  Installing BeeHive for $OS_KEY/$ARCH_KEY..."

# ── Fetch latest release version ───────────────────────────────────────────────
VERSION=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
  | grep '"tag_name"' \
  | sed 's/.*"tag_name":[[:space:]]*"\(.*\)".*/\1/')

if [ -z "$VERSION" ]; then
  echo "  Failed to fetch latest release from GitHub." && exit 1
fi

FILE="beehive-$VERSION-$PLATFORM.tar.gz"
URL="https://github.com/$REPO/releases/download/$VERSION/$FILE"

echo "  Downloading BeeHive $VERSION..."

# ── Download & extract ─────────────────────────────────────────────────────────
mkdir -p "$BIN_DIR"
TMP=$(mktemp -d)

curl -fsSL "$URL" -o "$TMP/$FILE"
tar -xzf "$TMP/$FILE" -C "$TMP"
mv "$TMP/beehive/beehive" "$BIN_DIR/beehive"
chmod +x "$BIN_DIR/beehive"
rm -rf "$TMP"

# ── Add to PATH ────────────────────────────────────────────────────────────────
PATH_LINE='export PATH="$HOME/.beehive/bin:$PATH"'

add_to_shell() {
  local rc="$1"
  if [ -f "$rc" ] && ! grep -q '\.beehive/bin' "$rc" 2>/dev/null; then
    printf '\n# BeeHive\n%s\n' "$PATH_LINE" >> "$rc"
  fi
}

add_to_shell "$HOME/.bashrc"
add_to_shell "$HOME/.zshrc"
add_to_shell "$HOME/.profile"

export PATH="$BIN_DIR:$PATH"

# ── Done ───────────────────────────────────────────────────────────────────────
echo ""
echo "  BeeHive $VERSION installed to $BIN_DIR"
echo ""
echo "  Restart your terminal, then run:"
echo "    beehive setup"
echo ""
