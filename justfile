# List all recipes
default:
	just --list --unsorted

# Build application
cargo-build:
	cargo build --release --locked --target x86_64-unknown-linux-musl

# Build application (default target)
cargo-build-no-target:
	cargo build --release --locked

# Clippy check
cargo-clippy-check:
	cargo clippy --release --no-deps --workspace --locked --tests -- -Dwarnings

# Cargo fmt check
cargo-fmt-check:
	cargo fmt --all --check

# Create release artifacts
release-artifacts:
	mkdir -p artifacts
	cp target/x86_64-unknown-linux-musl/release/health-check ./artifacts/health-check-x86_64-unknown-linux-musl

# Test 1: Will raise alert to slack
test1:
	cargo run --bin health-check -- --app-description "Indexer Raw Processor (k8s Testnet Mainnet)" --task-output-timeout 5 sleep-check -- --stdout-print --output-timeout 10

# Test 2: Will quit.
test2:
	# The sleep-check keeps writing to stderr every 6 seconds
	# The health-check keeps checking every 5 seconds
	cargo run --bin health-check -- --app-description "Indexer Raw Processor (ECS Testnet Mainnet)" --task-output-timeout 5 sleep-check -- --output-timeout 6

# Test 3: Same as test1 but doesn't write to stdout
test3:
	cargo run --bin health-check -- --app-description "Indexer Raw Processor (ECS Testnet Mainnet)" --task-output-timeout 5 sleep-check -- --output-timeout 4

# Test 4: Should keep printing. We give high threshold.
test4:
	cargo run --bin health-check -- --app-description "Indexer Raw Processor (ECS Testnet Mainnet)" --task-output-timeout 120 sleep-check -- --output-timeout 4
