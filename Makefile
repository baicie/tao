.PHONY: check fmt lint test doc conformance fuzz-smoke perf release-check security

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

release-check:
	cargo xtask release-check

security:
	cargo xtask security
