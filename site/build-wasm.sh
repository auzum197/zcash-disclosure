#!/bin/sh
# Rebuilds the wasm module and its JS glue into src/wasm. Run from site/: npm run wasm.
# The C part of the crypto stack (secp256k1) needs a clang with a wasm backend; Homebrew
# LLVM has one.
set -e
cd "$(dirname "$0")/.."

if [ -z "${CC_wasm32_unknown_unknown:-}" ] && [ -x "$(brew --prefix 2>/dev/null)/opt/llvm/bin/clang" ]; then
    CC_wasm32_unknown_unknown="$(brew --prefix)/opt/llvm/bin/clang"
    AR_wasm32_unknown_unknown="$(brew --prefix)/opt/llvm/bin/llvm-ar"
    export CC_wasm32_unknown_unknown AR_wasm32_unknown_unknown
fi

cargo build -p zdisclosure-wasm --release --target wasm32-unknown-unknown
wasm-bindgen --target web --out-dir site/src/wasm \
    target/wasm32-unknown-unknown/release/zdisclosure_wasm.wasm
echo "glue written to site/src/wasm/"
