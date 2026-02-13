.PHONY: build dev test clean

# Release build: Zig (ReleaseFast) + Rust (release)
build:
	cd zig && zig build -Doptimize=ReleaseFast
	cargo build --release --manifest-path Cargo.toml

# Debug build: Zig (Debug) + Rust (debug)
dev:
	cd zig && zig build
	cargo build --manifest-path Cargo.toml

# Run all tests
test:
	cd zig && zig build test
	cargo test --workspace

# Clean everything
clean:
	rm -rf zig/zig-out zig/.zig-cache target
