#!/usr/bin/env sh
set -eu

required_version=$(awk '
    $0 == "name = \"wasm-bindgen\"" {
        getline
        gsub(/\"/, "", $3)
        print $3
        exit
    }
' Cargo.lock)
installed_version=$(wasm-bindgen --version 2>/dev/null | awk '{ print $2 }' || true)

if [ "$installed_version" != "$required_version" ]; then
    echo "wasm-bindgen-cli $required_version is required by Cargo.lock." >&2
    echo "Install it with:" >&2
    echo "  cargo install wasm-bindgen-cli --version $required_version --locked --force" >&2
    exit 1
fi

cargo build --profile wasm-release --target wasm32-unknown-unknown
wasm-bindgen \
    --target web \
    --out-dir web/pkg \
    --out-name mrzavec \
    target/wasm32-unknown-unknown/wasm-release/mrzavec.wasm

if command -v wasm-opt >/dev/null 2>&1; then
    wasm-opt -Oz \
        --enable-mutable-globals \
        --enable-sign-ext \
        --enable-nontrapping-float-to-int \
        --enable-bulk-memory \
        --enable-reference-types \
        --enable-multivalue \
        -o web/pkg/mrzavec_bg.wasm \
        web/pkg/mrzavec_bg.wasm
else
    echo "wasm-opt not found; skipping the size pass (install binaryen)." >&2
fi

ls -la web/pkg/mrzavec_bg.wasm
echo "Built web/pkg. Serve the repository root over HTTP and open web/."
