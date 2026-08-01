.PHONY: check fmt lint test doc conformance fuzz-smoke perf storage-kernel nir-artifact release-check security bootstrap-contract

check:
	cargo xtask check

fmt:
	cargo xtask fmt

lint:
	cargo xtask lint

test:
	cargo xtask test

doc:
	cargo xtask doc

conformance:
	cargo xtask conformance

fuzz-smoke:
	cargo xtask fuzz-smoke

perf:
	cargo xtask perf

storage-kernel:
	cargo xtask storage-kernel

nir-artifact:
	cargo xtask nir-artifact

release-check:
	cargo xtask release-check

security:
	cargo xtask security

bootstrap-contract:
	cargo xtask bootstrap-contract
