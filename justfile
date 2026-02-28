default:
    @just --list

build:
    cargo +esp build -p ossm-alt-m57aim --target xtensa-esp32s3-none-elf -Z build-std=alloc,core --release

flash:
    cargo +esp build -p ossm-alt-m57aim --target xtensa-esp32s3-none-elf -Z build-std=alloc,core --release
    espflash flash --monitor target/xtensa-esp32s3-none-elf/release/ossm-alt-m57aim

# Build all targets
build-all: build
