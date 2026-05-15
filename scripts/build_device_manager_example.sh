#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

EXAMPLE_NAME="${1:-device_manager_example}"
LINUX_ARTIFACT="target/release/examples/${EXAMPLE_NAME}"
WINDOWS_ARTIFACT="target/x86_64-pc-windows-gnu/release/examples/${EXAMPLE_NAME}.exe"

ensure_cmd() {
    local cmd="$1"
    local install_hint="$2"

    if ! command -v "$cmd" >/dev/null 2>&1; then
        echo "Error: '$cmd' not found. ${install_hint}"
        exit 1
    fi
}

echo "==> Checking required tools"
ensure_cmd cargo "Install Rust with rustup: https://rustup.rs"
ensure_cmd rustup "Install Rust with rustup: https://rustup.rs"
ensure_cmd curl "Install curl from your distro package manager."
ensure_cmd tar "Install tar from your distro package manager."

if ! command -v cargo-zigbuild >/dev/null 2>&1; then
    echo "==> Installing cargo-zigbuild"
    cargo install cargo-zigbuild
fi

if ! rustup target list --installed | grep -q '^x86_64-pc-windows-gnu$'; then
    echo "==> Installing target x86_64-pc-windows-gnu"
    rustup target add x86_64-pc-windows-gnu
fi

mkdir -p "$HOME/.local/bin" "$HOME/.local/opt"

if ! command -v zig >/dev/null 2>&1; then
    echo "==> Installing local Zig toolchain (0.13.0)"
    cd "$HOME/.local/opt"
    if [[ ! -d zig-linux-x86_64-0.13.0 ]]; then
        curl -L -o zig.tar.xz https://ziglang.org/download/0.13.0/zig-linux-x86_64-0.13.0.tar.xz
        tar -xf zig.tar.xz
    fi
    ln -sf "$HOME/.local/opt/zig-linux-x86_64-0.13.0/zig" "$HOME/.local/bin/zig"
fi

export PATH="$HOME/.local/bin:$PATH"

cd "$ROOT_DIR"

echo "==> Building Linux release example: ${EXAMPLE_NAME}"
cargo build --release --example "$EXAMPLE_NAME"

echo "==> Building Windows release example: ${EXAMPLE_NAME}.exe"
cargo zigbuild --release --example "$EXAMPLE_NAME" --target x86_64-pc-windows-gnu

echo
echo "Build completed successfully."
if [[ -f "$LINUX_ARTIFACT" ]]; then
    ls -lh "$LINUX_ARTIFACT"
fi
if [[ -f "$WINDOWS_ARTIFACT" ]]; then
    ls -lh "$WINDOWS_ARTIFACT"
fi

echo
echo "Linux:   $ROOT_DIR/$LINUX_ARTIFACT"
echo "Windows: $ROOT_DIR/$WINDOWS_ARTIFACT"
