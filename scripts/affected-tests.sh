#!/usr/bin/env bash
# Map the current diff to the narrowest set of tests that covers it.
#
#   scripts/affected-tests.sh [--base REF] [--run]
#
# Changed files = `git diff --name-only <merge-base(REF)>` plus the working
# tree (staged, unstaged and untracked). Each path is mapped to
#   * a `cargo test --lib <filter>` module prefix, and/or
#   * one `tests/<chunk>.rs` acceptance binary (run in release).
# Without --run the plan is printed; with --run it is executed, unit filters
# first, then the chunk binaries one at a time so a failure is attributable.
#
# The 800-line guard (`--test file_size`) is always included: it is fast and
# any src/ edit can trip it. Cargo.toml, src/lib.rs, src/sim/mod.rs and the
# shared sim primitives are "core": they fan out to every chunk binary.

set -euo pipefail

base="main"
run=0
while [ "$#" -gt 0 ]; do
    case "$1" in
        --base) base="$2"; shift 2 ;;
        --run) run=1; shift ;;
        -h|--help) sed -n '2,17p' "$0"; exit 0 ;;
        *) echo "unknown argument: $1" >&2; exit 2 ;;
    esac
done

cd "$(git rev-parse --show-toplevel)"

merge_base="$(git merge-base "$base" HEAD 2>/dev/null || git rev-parse HEAD)"
changed="$( { git diff --name-only "$merge_base"; git diff --name-only; git diff --name-only --cached; git ls-files --others --exclude-standard; } | sort -u )"

if [ -z "$changed" ]; then
    echo "no changes relative to $base; nothing to run"
    exit 0
fi

declare -A units=() chunks=()
core=0
features=""

for f in $changed; do
    case "$f" in
        Cargo.toml|Cargo.lock|src/lib.rs|src/sim/mod.rs|src/sim/rng.rs|src/sim/time.rs|src/sim/spatial.rs|src/sim/geom.rs|src/sim/species.rs|src/sim/events.rs|src/sim/creatures.rs|src/sim/world.rs|src/sim/world/*)
            core=1 ;;
        src/main.rs)                              chunks[headless]=1 ;;
        src/sim/save.rs)                          units[sim::save]=1; chunks[headless]=1 ;;
        src/sim/disease*)                         units[sim::disease]=1; chunks[disease]=1 ;;
        src/sim/genetics*|src/sim/lineage.rs)     units[sim::genetics]=1; units[sim::lineage]=1; chunks[evolution]=1 ;;
        src/sim/predation.rs|src/sim/behavior/hunt.rs|src/sim/behavior/threat.rs)
            units[sim::predation]=1; units[sim::behavior]=1; chunks[predators]=1 ;;
        src/sim/behavior/migration.rs|src/sim/behavior/tests_migration.rs)
            units[sim::behavior]=1; chunks[predators]=1 ;;
        src/sim/behavior*)                        units[sim::behavior]=1; chunks[herbivores]=1 ;;
        src/sim/ecology.rs)                       units[sim::ecology]=1; chunks[ecology]=1 ;;
        src/sim/params*)                          units[sim::params]=1; chunks[headless]=1 ;;
        src/sim/stats*)                           units[sim::stats]=1 ;;
        src/sim/*)                                units[sim]=1 ;;
        src/ai/*|scripts/fake-gateway.js|tests/fixtures/ai/*)
            units[ai]=1; chunks[headless]=1; features="--features ai" ;;
        src/ui/*)                                 units[ui]=1 ;;
        src/widgets/*)                            units[widgets]=1 ;;
        src/theme.rs)                             units[theme]=1; units[ui]=1 ;;
        src/glyphs.rs)                            units[glyphs]=1; units[ui]=1 ;;
        tests/*.rs)                               chunks[$(basename "$f" .rs)]=1 ;;
        *) ;;
    esac
done

if [ "$core" -eq 1 ]; then
    for c in headless ecology herbivores predators evolution disease; do chunks[$c]=1; done
fi
# file_size is always cheap and always relevant.
chunks[file_size]=1

# A broad filter subsumes its narrower siblings (sim covers sim::disease).
if [ -n "${units[sim]:-}" ]; then
    for k in "${!units[@]}"; do case "$k" in sim::*) unset 'units[$k]' ;; esac; done
fi

echo "changed files:"
printf '  %s\n' $changed
echo
echo "plan:"
if [ "$core" -eq 1 ]; then
    echo "  core files changed: the whole lib suite is relevant (just test-unit-all, ~95 s, ask the user)"
fi
for u in $(printf '%s\n' "${!units[@]}" | sort); do
    echo "  cargo test $features --lib $u"
done
for c in $(printf '%s\n' "${!chunks[@]}" | sort); do
    case "$c" in
        predators|evolution|disease|sweep) echo "  cargo test --release $features --test $c" ;;
        *)                                 echo "  cargo test $features --test $c" ;;
    esac
done

[ "$run" -eq 1 ] || exit 0

echo
status=0
for u in $(printf '%s\n' "${!units[@]}" | sort); do
    echo "==> cargo test $features --lib $u"
    # shellcheck disable=SC2086
    cargo test $features --lib "$u" || status=1
done
for c in $(printf '%s\n' "${!chunks[@]}" | sort); do
    case "$c" in
        predators|evolution|disease|sweep)
            echo "==> cargo test --release $features --test $c"
            # shellcheck disable=SC2086
            cargo test --release $features --test "$c" || status=1 ;;
        *)
            echo "==> cargo test $features --test $c"
            # shellcheck disable=SC2086
            cargo test $features --test "$c" || status=1 ;;
    esac
done
exit "$status"
