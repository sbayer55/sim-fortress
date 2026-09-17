# Sim Fortress task runner. `just` lists the recipes.
#
# Test tiers (cheapest first). Agents: reach for the narrowest tier that
# covers the change; the full suite is CI's job and runs only when a human
# asks for it. A PreToolUse hook (scripts/hooks/guard-cargo-test.sh) rejects
# bare `cargo test` for the same reason.
#
#   recipe                     what it runs                          cost
#   ───────────────────────── ──────────────────────────────────── ────────
#   just check                 clippy + 800-line guard              seconds
#   just test-unit FILTER      cargo test --lib FILTER              seconds–tens of s
#   just test-chunk NAME       one tests/NAME.rs binary in release  ~1–3 min
#   just test-affected         unit filters + chunks for the diff   varies
#   just test-unit-all         whole lib suite (~95 s)              gated: user asks
#   just test-full             clippy + everything, release chunks  gated: user asks

set shell := ["bash", "-euo", "pipefail", "-c"]

default:
    @just --list --unsorted

# Fast gate: 0 clippy warnings + no src file over 800 lines. Run this always.
check:
    touch src/lib.rs
    cargo clippy --all-targets
    cargo clippy --all-targets --features ai
    cargo test --test file_size

# Unit tests for one module path or test name, e.g. `just test-unit sim::disease`.
test-unit FILTER:
    cargo test --lib {{FILTER}}

# Unit tests with the `ai` feature compiled in (C9). `ai::` tests need `node` on PATH.
test-unit-ai FILTER:
    cargo test --features ai --lib {{FILTER}}

# One tests/*.rs binary in release (predators, evolution, disease, ...). Slow: run in the background.
test-chunk NAME:
    cargo test --release --test {{NAME}}

# Print the unit filters and chunk binaries the current diff touches.
test-affected-plan BASE="main":
    scripts/affected-tests.sh --base {{BASE}}

# Run what the current diff touches (unit filters, then chunks in release).
test-affected BASE="main":
    scripts/affected-tests.sh --base {{BASE}} --run

# Whole lib suite. ~95 s (save round-trip dominates). Only when the user asks.
test-unit-all:
    cargo test --lib

# Everything: clippy, lib suite, every chunk binary in release. Minutes. Only when the user asks.
test-full:
    touch src/lib.rs
    cargo clippy --all-targets
    cargo test --lib
    cargo test --test file_size --test headless --test ecology --test herbivores
    cargo test --release --test predators --test evolution --test disease
    cargo test --features ai --lib ai
    cargo test --features ai --lib sim::tests::checksum_is_fnv_stable
    cargo test --features ai --test headless

# Refresh docs/screens/renders/*.txt after a UI change.
renders:
    cargo test --lib -- --ignored regenerate_screen_renders
