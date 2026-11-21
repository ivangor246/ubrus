.DEFAULT_GOAL := help
.NOTPARALLEL:
CARGO ?= cargo
export CARGO_BUILD_JOBS := 1
# Serialize independent make invocations as well as targets in a single invocation.
rust = mkdir -p target && flock target/ubrus-cargo.lock $(CARGO)
.PHONY: help build release format format-check lint typecheck deb snap extension
help:
	@echo "make build | release | format | check | deb | snap | extension"
build:
	$(rust) build --workspace --locked
release: extension
	$(rust) build --workspace --release --locked
	@mkdir -p dist
	@set -eu; tmp=$$(mktemp dist/.ubrus.XXXXXX); trap 'rm -f "$$tmp"' EXIT; \
		install -m 755 target/release/ubrus "$$tmp"; mv -fT "$$tmp" dist/ubrus
format:
	$(rust) fmt --all
format-check:
	$(rust) fmt --all -- --check
lint:
	$(rust) clippy --workspace --all-targets --locked -- -D warnings
typecheck:
	$(rust) check --workspace --all-targets --locked
extension:
	bash packaging/extension.sh
deb: release extension
	bash packaging/deb/build.sh
snap: release extension
	@set -eu; stage=$$(mktemp -d dist/.snap.XXXXXX); trap 'rm -rf "$$stage"' EXIT; \
	version=$$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml); \
	package="ubrus_$${version}_$$(dpkg --print-architecture).snap"; \
	if command -v snapcraft >/dev/null; then \
		flock target/ubrus-cargo.lock snapcraft pack --output "$$stage/$$package"; \
	else /usr/bin/python3 packaging/snap/build.py "$$stage/$$package"; fi; \
	mv "$$stage/$$package" dist/

include tests/targets.mk
