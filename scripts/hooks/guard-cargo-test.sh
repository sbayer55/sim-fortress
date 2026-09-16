#!/usr/bin/env bash
# Claude Code PreToolUse hook (matcher: Bash). Reads the tool call as JSON on
# stdin and vetoes test invocations that would run far more than the change
# needs. The tiers it steers towards live in the justfile and AGENTS.md.
#
#   deny  cargo test                      (every target, lib + all chunks)
#   deny  cargo test --lib                (unfiltered ~95 s lib suite)
#   deny  cargo test --workspace|--all-targets
#   deny  cargo test --test <slow chunk>  without --release
#   ask   just test-full / just test-unit-all   (the user must confirm)
#   allow everything else: --lib <filter>, --test <one chunk>, --no-run,
#         --example, --bin, --doc, -- --ignored <name>, ...
#
# Exit 0 with no output = no opinion. Never exits non-zero: a broken hook
# must not block unrelated commands.

set -u

command -v jq >/dev/null 2>&1 || exit 0
cmd="$(jq -r '.tool_input.command // empty' 2>/dev/null)" || exit 0
[ -n "$cmd" ] || exit 0

deny() {
    jq -cn --arg r "$1" '{hookSpecificOutput:{hookEventName:"PreToolUse",permissionDecision:"deny",permissionDecisionReason:$r}}'
    exit 0
}
ask() {
    jq -cn --arg r "$1" '{hookSpecificOutput:{hookEventName:"PreToolUse",permissionDecision:"ask",permissionDecisionReason:$r}}'
    exit 0
}

tiers='Use the narrowest tier instead: `just check` (clippy + file_size), `just test-unit <module::path>` (cargo test --lib <filter>), `just test-chunk <name>` (one tests/*.rs binary in release, run in the background), or `just test-affected` (derived from the diff). The whole lib suite (`just test-unit-all`, ~95 s) and the full suite (`just test-full`) run only when the user explicitly asks for them.'

# Gated recipes: the user confirms each time.
if printf '%s' "$cmd" | grep -Eq '(^|[;&|[:space:]])just[[:space:]]+(test-full|test-unit-all)([[:space:]]|$)'; then
    ask "This runs the full suite / whole lib suite (minutes). Confirm only if the user asked for it."
fi

# Drop heredoc bodies and replace quoted strings with a placeholder, so text
# that merely mentions a test command (a docs edit, an echo, a commit message)
# is not mistaken for one. A quoted filter such as "sim::x" still counts as a
# filter because the placeholder is a non-flag token.
scan="$(printf '%s\n' "$cmd" | python3 -c '
import re, sys
s = sys.stdin.read()
s = re.sub(r"<<-?\s*([\"\x27]?)(\w+)\1.*?\n\2(?:\n|$)", " ", s, flags=re.S)
s = re.sub(r"\"(?:[^\"\\\\]|\\\\.)*\"", "Q", s)
s = re.sub(r"\x27[^\x27]*\x27", "Q", s)
sys.stdout.write(s)
' 2>/dev/null)" || scan="$cmd"
[ -n "$scan" ] || scan="$cmd"

# Examine every `cargo [+toolchain] test ...` segment of a compound command.
printf '%s\n' "$scan" | grep -oE '(^|[;&|[:space:]])cargo([[:space:]]+\+[^[:space:]]+)?[[:space:]]+(test|t)([[:space:]][^;&|]*)?' | while IFS= read -r seg; do
    # Tokenise on whitespace (quoting is not needed for these flags).
    read -ra tok <<<"$seg"
    lib=0 target=0 release=0 norun=0 broad=0 filter=0 after_dashes=0 skipnext=0
    slow_chunk=""
    seen_test=0
    for t in "${tok[@]}"; do
        if [ "$seen_test" -eq 0 ]; then
            case "$t" in test|t) seen_test=1 ;; esac
            continue
        fi
        if [ "$skipnext" -eq 1 ]; then
            skipnext=0
            [ -n "$slow_chunk" ] && [ "$slow_chunk" = "?" ] && slow_chunk="$t"
            continue
        fi
        case "$t" in
            --) after_dashes=1 ;;
            --lib) lib=1 ;;
            --no-run) norun=1 ;;
            --release|-r) release=1 ;;
            --workspace|--all-targets|--all|--tests|--benches|--examples|--bins) broad=1 ;;
            --test=*)  target=1; case "${t#--test=}" in predators|evolution|disease|sweep) slow_chunk="${t#--test=}" ;; esac ;;
            --test)    target=1; slow_chunk="?"; skipnext=1 ;;
            --example=*|--bin=*|--doc|--bench=*) target=1 ;;
            --example|--bin|--bench) target=1; skipnext=1 ;;
            --profile=*|--features=*|--package=*|--jobs=*|--target=*|--test-threads=*|--color=*|--format=*|--logfile=*) ;;
            --profile|--features|-p|--package|-j|--jobs|--target|--test-threads|--color|--format|--logfile|-Z) skipnext=1 ;;
            --skip=*) filter=1 ;;
            --skip) filter=1; skipnext=1 ;;
            --ignored|--include-ignored) ;;
            -*) ;;
            *) filter=1 ;;
        esac
    done
    case "$slow_chunk" in predators|evolution|disease|sweep) ;; *) slow_chunk="" ;; esac
    [ "$seen_test" -eq 1 ] || continue
    [ "$norun" -eq 1 ] && continue

    if [ "$broad" -eq 1 ]; then
        deny "\`$seg\` runs every test target. $tiers"
    fi
    if [ "$target" -eq 0 ] && [ "$lib" -eq 0 ]; then
        deny "Bare \`cargo test\` runs the lib suite (~95 s) plus every acceptance binary (multi-year sims, minutes in debug). $tiers"
    fi
    if [ "$lib" -eq 1 ] && [ "$target" -eq 0 ] && [ "$filter" -eq 0 ]; then
        deny "\`cargo test --lib\` with no filter is the whole ~95 s unit suite. Pass the module path of what you changed, e.g. \`cargo test --lib sim::disease\` or \`just test-unit ui\`. $tiers"
    fi
    if [ -n "$slow_chunk" ] && [ "$release" -eq 0 ]; then
        deny "tests/$slow_chunk.rs runs multi-year simulations and is debug-hostile. Use \`cargo test --release --test $slow_chunk\` (\`just test-chunk $slow_chunk\`), in the background."
    fi
done
exit 0
