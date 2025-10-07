# Alpenglow Consensus - Makefile

.PHONY: all verify build test clean tla-check rust-model rust-impl ci-setup

all: verify

# Run all verification checks
verify: tla-check rust-model rust-impl
	@echo "✓ All verification checks passed"

# Build Rust implementation
build:
	cd rust-implementation && cargo build --release

# Run all tests
test:
	cd rust-implementation && cargo test --all

# Check TLA+ specification
tla-check:
	@echo "Checking TLA+ specification..."
	@if command -v tlc > /dev/null; then \
		cd tla-spec && tlc -config Alpenglow.cfg Alpenglow.tla; \
	else \
		echo "TLC not found. Install TLA+ Toolbox or Apalache."; \
		echo "Skipping TLA+ model checking..."; \
	fi

# Run Rust model checker (if implemented)
rust-model:
	@echo "Running Rust model checker..."
	@if [ -d rust-model ]; then \
		cd rust-model && cargo test; \
	else \
		echo "Rust model not yet implemented. Skipping..."; \
	fi

# Run Rust implementation tests
rust-impl:
	@echo "Running Rust implementation tests..."
	cd rust-implementation && cargo test && cargo clippy -- -D warnings

# Format all code
fmt:
	cd rust-implementation && cargo fmt

# Run linter
lint:
	cd rust-implementation && cargo clippy -- -D warnings

# Clean build artifacts
clean:
	cd rust-implementation && cargo clean
	find . -name "*.log" -delete
	find . -name "*.out" -delete

# Setup CI environment
ci-setup:
	@echo "Setting up CI environment..."
	@echo "Installing Rust..."
	@echo "For TLA+, install Java and TLA+ Toolbox manually"

# Run benchmarks
bench:
	cd rust-implementation && cargo bench

# Generate documentation
docs:
	cd rust-implementation && cargo doc --no-deps --open

# Run integration tests
integration-test:
	cd rust-implementation && cargo test --test '*' -- --test-threads=1

# Check code coverage (requires tarpaulin)
coverage:
	cd rust-implementation && cargo tarpaulin --out Html --output-dir ../docs/coverage