#!/bin/sh
# Install loki from GitHub releases.
#
#   curl -fsSL https://raw.githubusercontent.com/simonspoon/loki/main/install.sh | sh
#
# Env: LOKI_VERSION (default: latest), LOKI_INSTALL_DIR (default: /usr/local/bin)
set -eu

REPO=simonspoon/loki
VERSION=${LOKI_VERSION:-latest}
DIR=${LOKI_INSTALL_DIR:-/usr/local/bin}

die() { echo "install: $*" >&2; exit 1; }

[ "$(uname -s)" = Darwin ] || die "loki is macOS-only (got $(uname -s))"

case "$(uname -m)" in
  arm64)  ASSET=loki-darwin-arm64 ;;
  x86_64) ASSET=loki-darwin-amd64 ;;
  *)      die "unsupported architecture $(uname -m)" ;;
esac

if [ "$VERSION" = latest ]; then
  URL="https://github.com/$REPO/releases/latest/download/$ASSET"
else
  URL="https://github.com/$REPO/releases/download/v${VERSION#v}/$ASSET"
fi

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

echo "Downloading $ASSET ($VERSION)..."
curl -fSL --proto '=https' --tlsv1.2 "$URL" -o "$TMP/loki" \
  || die "download failed: $URL"
chmod +x "$TMP/loki"
"$TMP/loki" --version >/dev/null 2>&1 || die "downloaded binary does not run"

# sudo only when the target actually needs it
SUDO=
[ -w "$DIR" ] || { [ -d "$DIR" ] && SUDO=sudo; }
[ -d "$DIR" ] || $SUDO mkdir -p "$DIR" 2>/dev/null || { SUDO=sudo; $SUDO mkdir -p "$DIR"; }
[ -n "$SUDO" ] && echo "Installing to $DIR (needs sudo)..."
$SUDO install -m 755 "$TMP/loki" "$DIR/loki" || die "could not install to $DIR"

echo "Installed $("$DIR/loki" --version) -> $DIR/loki"

case ":$PATH:" in
  *":$DIR:"*) ;;
  *) echo "Note: $DIR is not on your PATH. Add it:"
     echo "  echo 'export PATH=\"$DIR:\$PATH\"' >> ~/.zshrc" ;;
esac

cat <<'EOF'

Next: grant accessibility permission (one-time)
  loki check-permission
  loki request-permission
EOF
