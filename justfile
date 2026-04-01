set windows-shell := ["powershell.exe", "-NoLogo", "-c", "if (Test-Path \"$env:USERPROFILE\\export-esp.ps1\") { . \"$env:USERPROFILE\\export-esp.ps1\" };"]
set shell := ["bash", "-c", ". $HOME/export-esp.sh 2>/dev/null; eval \"$0\""]
set dotenv-load := true

default:
    @just --list

# OSSM Alt (ESP32-S3)
[working-directory: 'firmware/ossm-alt']
build-ossm-alt:
    cargo +esp build --release

[working-directory: 'firmware/ossm-alt']
flash-ossm-alt:
    cargo +esp run --release

# Waveshare ESP32-S3-RS485-CAN
[working-directory: 'firmware/waveshare']
build-waveshare:
    cargo +esp build --release

[working-directory: 'firmware/waveshare']
flash-waveshare:
    cargo +esp run --release

# Seeed Studio XIAO ESP32-S3
[working-directory: 'firmware/seeed-xiao']
build-seeed-xiao:
    cargo +esp build --release

[working-directory: 'firmware/seeed-xiao']
flash-seeed-xiao:
    cargo +esp run --release

# OSSM Reference (ESP32)
[working-directory: 'firmware/ossm-reference']
build-ossm-reference:
    cargo +esp build --release

[working-directory: 'firmware/ossm-reference']
flash-ossm-reference:
    cargo +esp run --release


# WASM Simulator
[working-directory: 'firmware/sim-wasm']
build-wasm:
    cargo +stable build --release
    wasm-bindgen --target web --out-dir pkg target/wasm32-unknown-unknown/release/sim_wasm.wasm
    wasm-opt -O --all-features -o pkg/sim_wasm_bg.wasm pkg/sim_wasm_bg.wasm
    echo '{"name":"sim-wasm","type":"module","main":"sim_wasm.js","types":"sim_wasm.d.ts"}' > pkg/package.json

# Dev server (watches Rust sources and hot-reloads WASM)
[working-directory: 'apps/web-tools']
dev-patterns: build-wasm
    pnpm dev --host

# All
[parallel]
build-all: build-ossm-alt build-waveshare build-seeed-xiao build-ossm-reference build-wasm

# Check that all required tools are installed
[unix]
doctor:
    scripts/doctor.sh

[windows]
doctor:
    powershell.exe -NoLogo -ExecutionPolicy Bypass -File scripts/doctor.ps1

# Focus rust-analyzer on a firmware crate by generating editor settings from templates
[unix]
focus crate:
    scripts/focus.sh {{ crate }}

[windows]
focus crate:
    powershell.exe -NoLogo -ExecutionPolicy Bypass -File scripts/focus.ps1 -Crate {{ crate }}
