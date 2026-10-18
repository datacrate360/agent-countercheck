#!/bin/sh
# Install the countercheck command-line tool from GitHub Releases.
#
#   curl -fsSL https://raw.githubusercontent.com/datacrate360/countercheck/PROD/install.sh | sh
#
# Options (environment variables):
#   COUNTERCHECK_VERSION      version to install, e.g. 0.1.0 (default: the latest release)
#   COUNTERCHECK_INSTALL_DIR  where to put the binary (default: ~/.local/bin)
#   COUNTERCHECK_BASE_URL     download from here instead of GitHub (mirrors, testing)
#
# Every download is checked against the release's SHA256SUMS.txt before anything is installed.
set -eu

REPO="datacrate360/countercheck"
APP="countercheck"
INSTALL_DIR="${COUNTERCHECK_INSTALL_DIR:-$HOME/.local/bin}"

say() { printf '%s\n' "countercheck-install: $*"; }
die() { say "error: $*" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || die "this installer needs '$1'"; }

fetch() { # url dest
  if command -v curl >/dev/null 2>&1; then curl -fsSL --retry 3 -o "$2" "$1"
  elif command -v wget >/dev/null 2>&1; then wget -q -O "$2" "$1"
  else die "this installer needs curl or wget"; fi
}

sha256() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | cut -d' ' -f1
  else shasum -a 256 "$1" | cut -d' ' -f1; fi
}

need tar
need uname

case "$(uname -s)" in
  Darwin) platform="macos-universal" ;;
  Linux)
    case "$(uname -m)" in
      x86_64 | amd64) platform="linux-x86_64" ;;
      aarch64 | arm64) platform="linux-aarch64" ;;
      *) die "no build for Linux on $(uname -m) yet" ;;
    esac ;;
  MINGW* | MSYS* | CYGWIN*) die "Windows isn't supported yet; use WSL, or see https://github.com/$REPO" ;;
  *) die "no build for $(uname -s) yet" ;;
esac

version="${COUNTERCHECK_VERSION:-}"
if [ -z "$version" ]; then
  tmp_json="$(mktemp)"
  fetch "https://api.github.com/repos/$REPO/releases/latest" "$tmp_json" || die "could not look up the latest release"
  version="$(sed -n 's/.*"tag_name": *"v\{0,1\}\([^"]*\)".*/\1/p' "$tmp_json" | head -n 1)"
  rm -f "$tmp_json"
  [ -n "$version" ] || die "could not find the latest version; set COUNTERCHECK_VERSION"
fi
version="${version#v}"

base="${COUNTERCHECK_BASE_URL:-https://github.com/$REPO/releases/download/v$version}"
asset="$APP-$version-$platform.tar.gz"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT INT TERM

say "downloading $asset"
fetch "$base/$asset" "$work/$asset" || die "download failed: $base/$asset"
fetch "$base/SHA256SUMS.txt" "$work/SHA256SUMS.txt" || die "download failed: $base/SHA256SUMS.txt"

expected="$(awk -v f="$asset" '$2 == f || $2 == "*"f {print $1}' "$work/SHA256SUMS.txt")"
[ -n "$expected" ] || die "$asset is not listed in SHA256SUMS.txt"
actual="$(sha256 "$work/$asset")"
[ "$expected" = "$actual" ] || die "checksum mismatch for $asset (expected $expected, got $actual); nothing was installed"
say "checksum ok"

tar -xzf "$work/$asset" -C "$work"
[ -f "$work/$APP" ] || die "the archive has no $APP binary"
mkdir -p "$INSTALL_DIR"
install -m 0755 "$work/$APP" "$INSTALL_DIR/$APP" 2>/dev/null || { cp "$work/$APP" "$INSTALL_DIR/$APP"; chmod 0755 "$INSTALL_DIR/$APP"; }
if [ "$(uname -s)" = "Darwin" ]; then xattr -d com.apple.quarantine "$INSTALL_DIR/$APP" 2>/dev/null || true; fi

say "installed $("$INSTALL_DIR/$APP" --version) to $INSTALL_DIR/$APP"
case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *) say "add it to your PATH:  export PATH=\"$INSTALL_DIR:\$PATH\"" ;;
esac
say "next: cd your-project && countercheck init && countercheck verify"
