#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"

# Vercel has Rust pre-installed at /rust/bin
export PATH="/rust/bin:$PATH"

# Install wasm-pack and the WASM compile target
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
rustup target add wasm32-unknown-unknown

# Build the WASM package
cd "$REPO_ROOT"
wasm-pack build firmware/sim-wasm --target web

# Install dependencies and build the web app
cd "$REPO_ROOT/apps/simulator"
pnpm install
pnpm run build
