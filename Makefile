SHELL := /usr/bin/env bash
.DEFAULT_GOAL := test

.PHONY: check-toolchain fmt clippy unit sbf litesvm sdk audit test ci

check-toolchain:
	bash scripts/check-toolchain.sh

fmt:
	cargo fmt --all -- --check

clippy:
	cargo clippy --workspace --lib -- -D warnings

unit:
	cargo test --workspace --lib --locked

sbf:
	anchor build --ignore-keys --no-idl

litesvm: sbf
	anchor test --skip-build --skip-local-validator --skip-deploy

sdk:
	pnpm install --frozen-lockfile
	pnpm test

audit:
	cargo audit
	pnpm audit --audit-level high

test: check-toolchain fmt clippy unit litesvm sdk

ci: test audit
