SHELL := /usr/bin/env bash
.DEFAULT_GOAL := test

.PHONY: check-toolchain fmt clippy unit sbf litesvm sdk observer audit test ci \
	local-up deploy-local e2e local-down verifiable-build verify-deployment

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
	cargo build-sbf --manifest-path test-programs/mock_multisig/Cargo.toml --sbf-out-dir target/deploy

litesvm: sbf
	anchor test --skip-build --skip-local-validator --skip-deploy

sdk:
	pnpm install --frozen-lockfile
	pnpm test

observer:
	cargo fmt --manifest-path observer/Cargo.toml -- --check
	cargo clippy --manifest-path observer/Cargo.toml --all-targets --locked -- -D warnings
	cargo test --manifest-path observer/Cargo.toml --locked

audit:
	cargo audit
	test -z "$$(cargo tree --manifest-path observer/Cargo.toml --locked -i rsa 2>/dev/null)"
	cargo audit --file observer/Cargo.lock --ignore RUSTSEC-2023-0071
	pnpm audit --audit-level high

test: check-toolchain fmt clippy unit litesvm sdk observer

ci: test audit

local-up: sbf
	docker compose up --build --detach

deploy-local: sbf
	docker compose up --detach --force-recreate validator

e2e: sbf
	docker compose up --build --detach --force-recreate validator observer prometheus
	node scripts/e2e-local.mjs
	bash scripts/wait-observer-event.sh

local-down:
	docker compose down

verifiable-build:
	bash scripts/verifiable-build.sh

verify-deployment:
	test -n "$(PROGRAM_ID)" || (echo "PROGRAM_ID is required" >&2; exit 2)
	cd programs/iron_vault && anchor verify -p iron_vault "$(PROGRAM_ID)"
