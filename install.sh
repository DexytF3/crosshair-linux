#!/usr/bin/env bash
#
# Crosshair Linux — installer
#
#   ./install.sh                Install for the current user (~/.local)
#   sudo PREFIX=/usr ./install.sh   Install system-wide
#   ./install.sh --uninstall    Remove the installed files
#   ./install.sh --help         Show this help
#
# The installer copies the compiled binary, the icon (when present) and an
# application menu shortcut. It never touches your existing settings.
#
set -euo pipefail

APP_NAME="Crosshair Linux"
BIN_NAME="crosshair-linux"
APP_ID="io.github.crosshairlinux.CrosshairLinux"
ICON_NAME="$APP_ID"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

PREFIX="${PREFIX:-$HOME/.local}"
BIN_DIR="${BINDIR:-$PREFIX/bin}"
APPS_DIR="${APPDIR:-$PREFIX/share/applications}"
ICON_ROOT="${ICONDIR:-$PREFIX/share/icons/hicolor}"
ICON_PNG_DIR="$ICON_ROOT/512x512/apps"
ICON_SVG_DIR="$ICON_ROOT/scalable/apps"

BIN_DEST="$BIN_DIR/$BIN_NAME"
DESKTOP_DEST="$APPS_DIR/$APP_ID.desktop"
LEGACY_DESKTOP="$APPS_DIR/$BIN_NAME.desktop"
ICON_PNG_DEST="$ICON_PNG_DIR/$ICON_NAME.png"
ICON_SVG_DEST="$ICON_SVG_DIR/$ICON_NAME.svg"
LEGACY_ICON_PNG="$ICON_PNG_DIR/$BIN_NAME.png"
LEGACY_ICON_SVG="$ICON_SVG_DIR/$BIN_NAME.svg"

BOLD=$'\033[1m'; GREEN=$'\033[32m'; YELLOW=$'\033[33m'; RED=$'\033[31m'; BLUE=$'\033[34m'; RESET=$'\033[0m'
info() { echo "${BLUE}::${RESET} $*"; }
ok()   { echo "${GREEN}✓${RESET} $*"; }
warn() { echo "${YELLOW}!${RESET} $*"; }
fail() { echo "${RED}✗${RESET} $*" >&2; exit 1; }

usage() {
    sed -n '2,10p' "$0" | sed 's/^# \{0,1\}//'
    exit 0
}

refresh_caches() {
    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database "$APPS_DIR" >/dev/null 2>&1 || true
    fi
    if [ -d "$ICON_ROOT" ] && command -v gtk-update-icon-cache >/dev/null 2>&1; then
        gtk-update-icon-cache -f -t "$ICON_ROOT" >/dev/null 2>&1 || true
    fi
}

uninstall() {
    info "Removing $APP_NAME"
    rm -f "$BIN_DEST" "$DESKTOP_DEST" "$LEGACY_DESKTOP" \
          "$ICON_PNG_DEST" "$ICON_SVG_DEST" "$LEGACY_ICON_PNG" "$LEGACY_ICON_SVG"
    refresh_caches
    ok "Removed the binary, icon, and application shortcut."
    echo "Your settings in ~/.config/crosshair-linux were left untouched."
}

case "${1:-}" in
    -h|--help) usage ;;
    --uninstall) uninstall; exit 0 ;;
    "") ;;
    *) fail "Unknown option: $1 (try --help)" ;;
esac

echo
echo "${BOLD}  ┼  ${APP_NAME} installer${RESET}"
echo

# --- Find or build the binary ----------------------------------------------
if [ -x "$SCRIPT_DIR/target/release/$BIN_NAME" ]; then
    BIN_SRC="$SCRIPT_DIR/target/release/$BIN_NAME"
    ok "Using release build"
elif [ -x "$SCRIPT_DIR/target/debug/$BIN_NAME" ]; then
    BIN_SRC="$SCRIPT_DIR/target/debug/$BIN_NAME"
    warn "Release build not found, using debug build"
elif command -v cargo >/dev/null 2>&1; then
    info "No compiled binary found — building a release binary…"
    (cd "$SCRIPT_DIR" && cargo build --release)
    BIN_SRC="$SCRIPT_DIR/target/release/$BIN_NAME"
    [ -x "$BIN_SRC" ] || fail "Build finished but $BIN_SRC was not created."
    ok "Build complete"
else
    fail "No binary found and cargo is not installed. Build the app first: cargo build --release"
fi

# --- Install the binary -----------------------------------------------------
mkdir -p "$BIN_DIR"
rm -f "$BIN_DEST"
install -m 755 "$BIN_SRC" "$BIN_DEST"
ok "Binary installed to $BIN_DEST"

# --- Icon (only when the asset exists) -------------------------------------
if [ -f "$SCRIPT_DIR/assets/icon.png" ]; then
    mkdir -p "$ICON_PNG_DIR"
    install -m 644 "$SCRIPT_DIR/assets/icon.png" "$ICON_PNG_DEST"
    ok "Icon installed"
elif [ -f "$SCRIPT_DIR/assets/icon.svg" ]; then
    mkdir -p "$ICON_SVG_DIR"
    install -m 644 "$SCRIPT_DIR/assets/icon.svg" "$ICON_SVG_DEST"
    ok "Vector icon installed"
else
    warn "No app icon found in assets/ — the shortcut will use a generic icon"
fi

# Drop files from older installs so only one desktop entry/icon remains.
rm -f "$LEGACY_DESKTOP" "$LEGACY_ICON_PNG" "$LEGACY_ICON_SVG"

# --- Application menu shortcut ---------------------------------------------
mkdir -p "$APPS_DIR"
DESKTOP_TMP="$(mktemp "$APPS_DIR/.$APP_ID.desktop.XXXXXX")"
trap 'rm -f "$DESKTOP_TMP"' EXIT
cat > "$DESKTOP_TMP" <<EOF
[Desktop Entry]
Type=Application
Name=$APP_NAME
GenericName=Crosshair Overlay
Comment=Minimal crosshair overlay for Linux
Exec=$BIN_DEST
TryExec=$BIN_DEST
Icon=$ICON_NAME
Terminal=false
Categories=Game;
Keywords=crosshair;overlay;gaming;fps;
StartupNotify=true
StartupWMClass=$APP_ID
EOF
chmod 644 "$DESKTOP_TMP"
mv -f "$DESKTOP_TMP" "$DESKTOP_DEST"
trap - EXIT
ok "Application shortcut installed to $DESKTOP_DEST"

refresh_caches

# --- PATH hint --------------------------------------------------------------
case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *) warn "$BIN_DIR is not in your PATH. The application menu shortcut still works." ;;
esac

echo
echo "${GREEN}${BOLD}Done!${RESET} Launch ${BOLD}${APP_NAME}${RESET} from your application menu,"
echo "or run: ${BOLD}$BIN_DEST${RESET}"
echo "To uninstall: ./install.sh --uninstall"
echo
