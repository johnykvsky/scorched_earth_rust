# Simple shortcuts around cargo. Run `make help` for the list.

# rustup installs into ~/.cargo/bin; make sure it is found even if not on PATH.
export PATH := $(HOME)/.cargo/bin:$(PATH)

BIN := target/release/scorched

.PHONY: help run build dev check fmt clean

help:
	@echo "make run    - build optimised and start the game"
	@echo "make build  - build optimised binary ($(BIN))"
	@echo "make dev    - build and run a debug build (faster to compile)"
	@echo "make check  - type-check without building"
	@echo "make fmt    - format the source code"
	@echo "make clean  - remove build output"

run:
	cargo run --release

build:
	cargo build --release

dev:
	cargo run

check:
	cargo check

fmt:
	cargo fmt

clean:
	cargo clean
