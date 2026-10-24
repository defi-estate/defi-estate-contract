# Run the same checks as CI before you push.

.PHONY: check fix build test deny help

help:
	@echo "make check   fmt, clippy, test, wasm build and cargo-deny (same as CI)"
	@echo "make fix     cargo fmt"
	@echo "make build   build optimized wasm"
	@echo "make test    cargo test"
	@echo "make deny    cargo deny check (licenses and advisories); requires: cargo install cargo-deny"

check:
	cargo fmt --all -- --check
	cargo clippy --all-targets --all-features -- -D warnings
	cargo test
	cargo build --target wasm32v1-none --release
	cargo deny check

fix:
	cargo fmt --all

build:
	./scripts/build-contracts.sh

test:
	cargo test

# Check Rust dependencies for known vulnerabilities and disallowed licenses.
# Requires cargo-deny: cargo install cargo-deny
deny:
	cargo deny check
</br>
