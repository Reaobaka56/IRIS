#!/usr/bin/env bash
# ──────────────────────────────────────────────────────────────────────────
# build-rpm.sh — Build an .rpm package for IRIS Language
# ──────────────────────────────────────────────────────────────────────────
# Usage:
#   ./installer/linux/build-rpm.sh [--version 0.2.0] [--arch x86_64]
#
# Produces: installer/dist/iris-<version>-1.<arch>.rpm
# Requires: rpmbuild (from rpm-build package)
# ──────────────────────────────────────────────────────────────────────────
set -euo pipefail

VERSION="0.2.0"
ARCH="x86_64"
RELEASE="1"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Parse arguments
while [[ $# -gt 0 ]]; do
    case "$1" in
        --version) VERSION="$2"; shift 2 ;;
        --arch)    ARCH="$2";    shift 2 ;;
        *)         echo "Unknown argument: $1"; exit 1 ;;
    esac
done

# Map arch names for RPM
case "$ARCH" in
    x86_64|amd64)    RPM_ARCH="x86_64" ;;
    aarch64|arm64)   RPM_ARCH="aarch64" ;;
    *)               echo "Unsupported arch: $ARCH"; exit 1 ;;
esac

echo "Building IRIS .rpm package v${VERSION} (${RPM_ARCH})"

# ── Paths ─────────────────────────────────────────────────────────────────
DIST_DIR="$ROOT/installer/dist"
RPM_BUILD_ROOT="$ROOT/installer/dist/rpm-staging"
RPM_FILE="iris-${VERSION}-${RELEASE}.${RPM_ARCH}.rpm"

# ── Clean previous build ─────────────────────────────────────────────────
rm -rf "$RPM_BUILD_ROOT"
mkdir -p "$RPM_BUILD_ROOT"/{BUILD,RPMS,SOURCES,SPECS,SRPMS,BUILDROOT}
mkdir -p "$DIST_DIR"

# ── Build release binary (if not already built) ──────────────────────────
IRIS_BIN="$ROOT/target/release/iris"
if [[ ! -f "$IRIS_BIN" ]]; then
    echo "[1/4] Building release binary..."
    (cd "$ROOT" && cargo build --release)
else
    echo "[1/4] Using existing release binary."
fi

# ── Create source tarball ────────────────────────────────────────────────
echo "[2/4] Creating source tarball..."
SRC_DIR="$RPM_BUILD_ROOT/SOURCES/iris-${VERSION}"
mkdir -p "$SRC_DIR/bin"
mkdir -p "$SRC_DIR/stdlib"
mkdir -p "$SRC_DIR/examples"
mkdir -p "$SRC_DIR/doc"

cp "$IRIS_BIN" "$SRC_DIR/bin/iris"
cp -r "$ROOT/stdlib/"* "$SRC_DIR/stdlib/" 2>/dev/null || true
cp -r "$ROOT/examples/"* "$SRC_DIR/examples/" 2>/dev/null || true
cp "$ROOT/LICENSE" "$SRC_DIR/doc/"
cp "$ROOT/README.md" "$SRC_DIR/doc/"

(cd "$RPM_BUILD_ROOT/SOURCES" && tar czf "iris-${VERSION}.tar.gz" "iris-${VERSION}")
rm -rf "$SRC_DIR"

# ── Create spec file ─────────────────────────────────────────────────────
echo "[3/4] Creating RPM spec file..."
cat > "$RPM_BUILD_ROOT/SPECS/iris.spec" << SPECEOF
Name:           iris
Version:        ${VERSION}
Release:        ${RELEASE}%{?dist}
Summary:        IRIS programming language compiler and toolchain

License:        GPL-2.0-or-later
URL:            https://github.com/moon9t/iris
Source0:        iris-%{version}.tar.gz

# Pre-built binary — skip debug package and build steps
%global debug_package %{nil}
AutoReqProv:    no
Recommends:     clang lld

%description
IRIS (Intermediate Representation for Intelligent Systems) is a
programming language designed for machine learning and systems
programming. Includes compiler, interpreter, REPL, LSP server,
and DAP server.

%prep
%setup -q -n iris-%{version}

%install
mkdir -p %{buildroot}/usr/bin
mkdir -p %{buildroot}/usr/share/iris/stdlib
mkdir -p %{buildroot}/usr/share/iris/examples
mkdir -p %{buildroot}/usr/share/doc/iris

install -m 755 bin/iris %{buildroot}/usr/bin/iris
cp -r stdlib/* %{buildroot}/usr/share/iris/stdlib/ 2>/dev/null || true
cp -r examples/* %{buildroot}/usr/share/iris/examples/ 2>/dev/null || true
cp doc/LICENSE %{buildroot}/usr/share/doc/iris/
cp doc/README.md %{buildroot}/usr/share/doc/iris/

%files
%license /usr/share/doc/iris/LICENSE
%doc /usr/share/doc/iris/README.md
/usr/bin/iris
/usr/share/iris/

%post
echo ""
echo "  IRIS Language installed successfully!"
echo "  Run 'iris --version' to verify."
echo ""

%changelog
* $(date '+%a %b %d %Y') IRIS Language Project <iris@moon9t.dev> - ${VERSION}-${RELEASE}
- Release v${VERSION}
SPECEOF

# ── Build .rpm ────────────────────────────────────────────────────────────
echo "[4/4] Building .rpm package..."

if ! command -v rpmbuild &>/dev/null; then
    echo "Error: rpmbuild not found."
    echo "Install it with:"
    echo "  Fedora/RHEL: sudo dnf install rpm-build"
    echo "  Ubuntu:      sudo apt install rpm"
    exit 1
fi

rpmbuild \
    --define "_topdir $RPM_BUILD_ROOT" \
    --target "$RPM_ARCH" \
    -bb "$RPM_BUILD_ROOT/SPECS/iris.spec"

# Copy the built RPM to dist
find "$RPM_BUILD_ROOT/RPMS" -name '*.rpm' -exec cp {} "$DIST_DIR/" \;

# Cleanup staging
rm -rf "$RPM_BUILD_ROOT"

# Find the output file
RPM_OUT="$(find "$DIST_DIR" -name 'iris-*.rpm' -newer "$DIST_DIR" 2>/dev/null | head -1)"
if [[ -z "$RPM_OUT" ]]; then
    RPM_OUT="$DIST_DIR/$RPM_FILE"
fi

echo ""
echo "  Package ready: $RPM_OUT"
if [[ -f "$RPM_OUT" ]]; then
    SIZE_MB=$(du -m "$RPM_OUT" | awk '{print $1}')
    echo "  Size: ${SIZE_MB} MB"
fi
echo ""
echo "  Install with:"
echo "    sudo rpm -i $RPM_FILE"
echo "    # or"
echo "    sudo dnf install ./$RPM_FILE"
echo ""
