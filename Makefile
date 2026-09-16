EXE ?= neyrang
PROFILE ?= maxperf
TARGET_CPU ?= native
CARGO ?= cargo
TARGET_DIR ?= target

.PHONY: all quality

all:
	RUSTFLAGS="$(RUSTFLAGS) -C target-cpu=$(TARGET_CPU)" $(CARGO) build --locked --profile $(PROFILE)
	cp "$(TARGET_DIR)/$(PROFILE)/neyrang" "$(EXE)"
	chmod +x "$(EXE)"

quality:
	$(CARGO) fmt --check
	$(CARGO) fmt --manifest-path tools/nnue-reference/Cargo.toml --check
	$(CARGO) fmt --manifest-path tools/nnue-data/Cargo.toml --check
	$(CARGO) fmt --manifest-path tools/nnue-trainer/Cargo.toml --check
	$(CARGO) fmt --manifest-path tools/policy-trace/Cargo.toml --check
	$(CARGO) clippy --locked --all-targets --all-features -- -D warnings
	$(CARGO) clippy --manifest-path tools/nnue-reference/Cargo.toml --locked --all-targets -- -D warnings
	$(CARGO) clippy --manifest-path tools/nnue-data/Cargo.toml --locked --all-targets -- -D warnings
	$(CARGO) clippy --manifest-path tools/nnue-trainer/Cargo.toml --locked --no-default-features --all-targets -- -D warnings
	$(CARGO) clippy --manifest-path tools/policy-trace/Cargo.toml --locked --all-targets -- -D warnings
	$(CARGO) test --locked
	$(CARGO) test --locked --all-features
	$(CARGO) test --manifest-path tools/nnue-reference/Cargo.toml --locked
	$(CARGO) test --manifest-path tools/nnue-data/Cargo.toml --locked
	$(CARGO) test --manifest-path tools/nnue-trainer/Cargo.toml --locked --no-default-features
	$(CARGO) test --manifest-path tools/policy-trace/Cargo.toml --locked
