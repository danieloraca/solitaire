.PHONY: build run test clean

WASM_TARGET := target/wasm32-unknown-unknown/release/solitaire.wasm
WASM_DIST := dist/solitaire.wasm

build: $(WASM_DIST)
	cargo build --release

$(WASM_DIST): src/lib.rs Cargo.toml
	cargo build --lib --release --target wasm32-unknown-unknown
	mkdir -p dist
	cp $(WASM_TARGET) $(WASM_DIST)

run: build
	cargo run --release

test:
	cargo test

clean:
	cargo clean
	rm -f $(WASM_DIST)
