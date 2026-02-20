#!/bin/bash
# Setup script for WASM compilation of ast-surgeon.
#
# Prerequisites:
#   - Rust toolchain with wasm32-unknown-unknown target
#   - wasm-pack
#   - LLVM (with WASM backend) + wasi-libc
#
# On macOS, Apple's clang does NOT include the WASM target.
# You need Homebrew's LLVM instead.

set -euo pipefail

echo "=== ast-surgeon WASM build setup ==="

# 1. Install Rust WASM target
echo "Adding wasm32-unknown-unknown target..."
rustup target add wasm32-unknown-unknown

# 2. Install wasm-pack
if ! command -v wasm-pack &> /dev/null; then
    echo "Installing wasm-pack..."
    cargo install wasm-pack
else
    echo "wasm-pack already installed: $(wasm-pack --version)"
fi

# 3. Install LLVM with WASM backend (macOS)
if [[ "$OSTYPE" == "darwin"* ]]; then
    if ! brew list llvm &> /dev/null; then
        echo "Installing LLVM via Homebrew (includes WASM backend)..."
        echo "This may take a while (~1.5GB download)..."
        brew install llvm
    else
        echo "LLVM already installed via Homebrew"
    fi

    # Install wasi-libc for C standard library headers
    if ! brew list wasi-libc &> /dev/null; then
        echo "Installing wasi-libc..."
        brew install wasi-libc
    else
        echo "wasi-libc already installed"
    fi

    LLVM_PREFIX="$(brew --prefix llvm)"
    WASI_SYSROOT="$(brew --prefix wasi-libc)/share/wasi-sysroot"

    echo ""
    echo "=== Build environment ==="
    echo "LLVM: $LLVM_PREFIX"
    echo "WASI sysroot: $WASI_SYSROOT"
    echo ""
    echo "To build WASM, run:"
    echo ""
    echo "  CC_wasm32_unknown_unknown=\"${LLVM_PREFIX}/bin/clang\" \\"
    echo "  CFLAGS_wasm32_unknown_unknown=\"--sysroot=${WASI_SYSROOT} -I${WASI_SYSROOT}/include/wasm32-wasi -Wno-implicit-function-declaration\" \\"
    echo "  wasm-pack build crates/ast-surgeon-wasm --target nodejs --out-dir ../../packages/mcp-server/wasm"
else
    echo "On Linux, system clang usually supports WASM. If not, install clang >= 11."
    echo ""
    echo "To build WASM, run:"
    echo "  wasm-pack build crates/ast-surgeon-wasm --target nodejs --out-dir ../../packages/mcp-server/wasm"
fi

echo ""
echo "=== Setup complete ==="
