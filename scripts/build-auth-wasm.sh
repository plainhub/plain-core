#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
out="${1:-$root/../plain-desktop/src/lib/opaque/pkg}"
cd "$root"
if [[ "$(wasm-bindgen --version)" != "wasm-bindgen 0.2.129" ]]; then
  echo 'wasm-bindgen CLI 0.2.129 is required' >&2
  exit 1
fi
export RUSTFLAGS="${RUSTFLAGS:-} --remap-path-prefix=$root=/plain-core --remap-path-prefix=${CARGO_HOME:-$HOME/.cargo}=/cargo-home"
cargo rustc --lib --crate-type cdylib --no-default-features --features auth --manifest-path "$root/Cargo.toml" --locked -p plain-libs --release --target wasm32-unknown-unknown --target-dir "$root/target/auth-wasm"
wasm-bindgen "$root/target/auth-wasm/wasm32-unknown-unknown/release/plain_libs.wasm" --target web --out-dir "$out" --out-name plain_auth
