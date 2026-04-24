set windows-shell := ["powershell.exe", "-NoLogo", "-c", "if (Test-Path \"$env:USERPROFILE\\export-esp.ps1\") { . \"$env:USERPROFILE\\export-esp.ps1\" };"]
set shell := ["bash", "-c", ". $HOME/export-esp.sh 2>/dev/null; eval \"$0\""]
set dotenv-load := true

default:
    @just --list

# OSSM Alt (ESP32-S3). Pass motor=sim to swap in the simulated motor.
[working-directory: 'firmware/esp32s3']
build-ossm-alt motor="rs485":
    cargo +esp build --release --bin ossm-alt --features motor-{{ motor }}

[working-directory: 'firmware/esp32s3']
flash-ossm-alt motor="rs485":
    cargo +esp run --release --bin ossm-alt --features motor-{{ motor }}

# Waveshare ESP32-S3-RS485-CAN. Pass motor=sim to swap in the simulated motor.
[working-directory: 'firmware/esp32s3']
build-waveshare motor="rs485":
    cargo +esp build --release --bin waveshare --features motor-{{ motor }}

[working-directory: 'firmware/esp32s3']
flash-waveshare motor="rs485":
    cargo +esp run --release --bin waveshare --features motor-{{ motor }}

# Seeed Studio XIAO ESP32-S3. Pass motor=sim to swap in the simulated motor.
[working-directory: 'firmware/esp32s3']
build-seeed-xiao motor="rs485":
    cargo +esp build --release --bin seeed-xiao --features motor-{{ motor }}

[working-directory: 'firmware/esp32s3']
flash-seeed-xiao motor="rs485":
    cargo +esp run --release --bin seeed-xiao --features motor-{{ motor }}

# OSSM Reference (ESP32). Pass motor=sim to swap in the simulated motor.
[working-directory: 'firmware/esp32']
build-ossm-reference motor="stepdir":
    cargo +esp build --release --bin ossm-reference --features motor-{{ motor }}

[working-directory: 'firmware/esp32']
flash-ossm-reference motor="stepdir":
    cargo +esp run --release --bin ossm-reference --features motor-{{ motor }}


# WASM Simulator
[working-directory: 'bindings/web-simulator']
build-wasm-simulator:
    cargo +stable build --release
    wasm-bindgen --target web --out-dir pkg target/wasm32-unknown-unknown/release/web_simulator.wasm
    wasm-opt -O --all-features -o pkg/web_simulator_bg.wasm pkg/web_simulator_bg.wasm
    echo '{"name":"@ossm-rs/web-simulator","type":"module","main":"web_simulator.js","types":"web_simulator.d.ts"}' > pkg/package.json

# WASM Trajectory Recorder
[working-directory: 'bindings/trajectory-recorder']
build-wasm-recorder:
    cargo +stable build --release
    wasm-bindgen --target web --out-dir pkg target/wasm32-unknown-unknown/release/trajectory_recorder.wasm
    wasm-opt -O --all-features -o pkg/trajectory_recorder_bg.wasm pkg/trajectory_recorder_bg.wasm
    echo '{"name":"@ossm-rs/trajectory-recorder","type":"module","main":"trajectory_recorder.js","types":"trajectory_recorder.d.ts"}' > pkg/package.json

build-wasm: build-wasm-simulator build-wasm-recorder

# Dev server (watches Rust sources and hot-reloads WASM)
[working-directory: 'apps/web-tools']
web-tools: build-wasm
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
