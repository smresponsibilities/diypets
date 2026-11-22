#!/usr/bin/env bash
set -euo pipefail
OS="$(uname -s)"
if [ "$OS" = "Darwin" ]; then
  TARGET="aarch64-apple-darwin"
else
  TARGET="x86_64-unknown-linux-gnu"
fi
DIR="$HOME/.local/share/diypets-app"
URL="https://github.com/smresponsibilities/diypets/releases/latest/download/diypets-${TARGET}.tar.gz"
echo "Downloading DIYPets ($TARGET)..."
mkdir -p "$DIR"
curl -fL "$URL" | tar -xz --strip-components=1 -C "$DIR"
echo "Installed to $DIR — run: $DIR/diypets"