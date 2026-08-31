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
	$(CARGO) clippy --all-targets --all-features -- -D warnings
	$(CARGO) test
	$(CARGO) test --all-features
