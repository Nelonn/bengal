#!/bin/bash
set -e

# Detect OS
OS="$(uname -s)"

if [ -f /etc/os-release ]; then
    . /etc/os-release
    DISTRO_ID="${ID}"
    DISTRO_LIKE="${ID_LIKE:-}"
fi

REPO="cubekii/bengal"
LATEST=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
    | grep '"tag_name"' \
    | sed 's/.*"tag_name": *"\(.*\)".*/\1/')

ARCH=$(uname -m)

case "$ARCH" in
    x86_64)  ARCH="amd64" ;;
    aarch64) ARCH="arm64" ;;
esac

cat << "EOF"
██████╗ ███████╗███╗   ██╗ ██████╗  █████╗ ██╗
██╔══██╗██╔════╝████╗  ██║██╔════╝ ██╔══██╗██║
██████╔╝█████╗  ██╔██╗ ██║██║  ███╗███████║██║
██╔══██╗██╔══╝  ██║╚██╗██║██║   ██║██╔══██║██║
██████╔╝███████╗██║ ╚████║╚██████╔╝██║  ██║███████╗
╚═════╝ ╚══════╝╚═╝  ╚═══╝ ╚═════╝ ╚═╝  ╚═╝╚══════╝
EOF
echo -e "\033[0;32mWelcome to bengal installation script\033[0m"

TMPDIR=$(mktemp -d)
trap "rm -rf $TMPDIR" EXIT

if echo "$DISTRO_ID $DISTRO_LIKE" | grep -qiE "debian|ubuntu"; then
    echo "Detected Debian-based distro, installing .deb package..."
    curl -sSL -o "$TMPDIR/bengal.deb" "https://github.com/${REPO}/releases/download/${LATEST}/bengal_${LATEST#v}_${ARCH}.deb"
    sudo apt-get install -y "$TMPDIR/bengal.deb"
    echo "Bengal installed successfully!"
    exit 0
elif echo "$DISTRO_ID $DISTRO_LIKE" | grep -qiE "fedora|rhel|centos|suse"; then
    echo "Detected RPM-based distro, installing .rpm package..."
    curl -sSL -o "$TMPDIR/bengal.rpm" "https://github.com/${REPO}/releases/download/${LATEST}/bengal_${LATEST#v}_${ARCH}.rpm"
    sudo dnf install -y "$TMPDIR/bengal.rpm"
    echo "Bengal installed successfully!"
    exit 0
elif echo "$DISTRO_ID $DISTRO_LIKE" | grep -qiE "arch|manjaro|endeavour"; then
    echo "Detected Arch-based distro, installing .tar.gz package..."
    curl -sSL -o "$TMPDIR/bengal.tar.gz" "https://github.com/${REPO}/releases/download/${LATEST}/bengal_${LATEST#v}_${ARCH}.tar.gz"
    tar -xzf "$TMPDIR/bengal.tar.gz" -C "$TMPDIR"
    sudo cp "$TMPDIR/usr/bin/bengal" /usr/bin/bengal
    sudo chmod 755 /usr/bin/bengal
    echo "Bengal installed successfully!"
    exit 0
else
    echo "Unsupported distro: $DISTRO_ID" >&2
    echo "Please download manually from https://github.com/${REPO}/releases" >&2
    exit 1
fi