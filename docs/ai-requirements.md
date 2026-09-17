# AI feature requirements

Back to the [roadmap](chunks/README.md). Ideas that would use this: the
[AI section of the backlog](feature-ideas.md#ai-assisted-features-opt-in-via-a-local-bifrost-gateway).

> **Status: drafted 2026-09-16; the shared plumbing and the first two features shipped in
> [C9](chunks/c9-ai-designer-and-chronicle.md) the same day.** Amendments made while building
> are marked *(C9)* inline. This document is binding for every
> feature that sends a prompt to a language model. It states the rules (R1–R14), the
> configuration and architecture every feature shares, the checklist for building one, and the
> tests that pin the rules. A chunk doc for an AI feature references this file instead of
> restating it. Where this file and a chunk doc disagree, this file wins until it is amended.

## Goal

Let the game and the headless runner use a language model for narration, naming, analysis and
authoring inputs, **without changing what the simulation is**: a deterministic, offline, pure
core that a player can run on a laptop with no network, no account and no configuration. AI is
a layer on the outside that the player switches on, feature by feature, knowing what each one
sends and where.

## Definitions

- **AI feature.** Any code path that builds a prompt and sends it to a model. Listed in
  `[ai.features]` (below) by a stable snake_case id.
- **Gateway.** The one thing the game talks to: a [Bifrost](https://github.com/maximhq/bifrost)
  instance on the player's machine, reached over its OpenAI-compatible
  `POST /v1/chat/completions` endpoint. Default `http://localhost:8080/v1`.
- **Backend.** A provider behind the gateway — Ollama on the same machine, AWS Bedrock in the
  cloud, or anything else Bifrost routes to. The game never names a backend in code; it names a
  model string like `ollama/llama3.1:8b` or `bedrock/claude-sonnet`, and the prefix is the
  backend.
- **Upstream feature.** The model writes a file the sim already accepts as input: a species
  overlay, a params overlay, a scenario. Nondeterministic once, at authoring time; a plain
  deterministic input from then on.
- **Downstream feature.** The model reads sim state or events and produces text for the screen,
  a report file, or a decorative table in the save. Nothing flows back into the step.
- **Worker.** The single background thread that owns the HTTP client. Screens and the headless
  runner never call the gateway directly.

## The rules

Each rule names the test that pins it. A pull request that adds an AI feature must leave
every one of these tests green and must not weaken any of them.

**R1 — Opt in, twice.** AI is off unless the player sets `[ai] enabled = true`, and a feature
is off unless `[ai.features]` names a model for it. A missing `[ai]` table means off. The game
never probes the gateway port, reads a `BIFROST_URL`-style environment variable, or shows a
first-run prompt to discover or suggest enabling AI. A reachable gateway is not a signal.
*Pinned by:* `ui::tests::ai_off_makes_no_requests` (no config; the fake gateway records zero
requests across a session that visits every screen) and
`ui::tests::ai_master_on_without_features_makes_no_requests`.

**R2 — Optional at every level.** With AI off, unreachable, misconfigured, or compiled out,
the game and every headless flag that exists today behave exactly as they do now. Every
feature has a **non-AI fallback** that is the existing behaviour, named in its spec. An
unreachable gateway shows a dim `AI: offline` status and nothing else changes.
*Pinned by:* `ui::tests::ai_unreachable_reaches_map_and_steps` (base URL on a closed port; the
app reaches S01 and steps) and the existing screen render snapshots, which must not change when
AI is off.

**R3 — The step never sees the model.** No AI code under `src/sim`. `Sim::step` and everything
it calls cannot observe whether AI is on. `sim::tests::checksum_is_fnv_stable` must produce
`0x9b4ed39b8baac6f5` (the value `AGENTS.md` pins; an earlier draft quoted a stale one) with
the `ai` Cargo feature on and off. The existing purity scan in
`src/sim/mod.rs` keeps ratatui and hash maps out; the AI module lives beside `ui`, not under
`sim`, so the scan never sees it. The `[ai]` **config struct** may live with `UiParams` in
`sim::params` because it is plain data with no behaviour.
*Pinned by:* the checksum test run under both feature sets in `just check`, and
`tests/headless.rs::ai_flags_absent_runs_are_offline`.

**R4 — Data flows one way per feature.** A feature is upstream or downstream, never both in one
call. Upstream output is written to a file the player can read and edit, and enters the sim
through the same loader as a hand-written file (`--params`, the world-gen form). Downstream
output goes to the screen, a report file next to `summary.csv`, or a decorative save table
(R11). There is no third destination.
*Pinned by:* review; the feature spec's data-flow line (template below) must say which.

**R5 — One provider, one protocol.** The game links no provider SDK. It speaks OpenAI-style
chat completions to the gateway with a small blocking HTTP client, and nothing else. Routing,
retries, provider fallbacks, key rotation, caching and cost tracking are Bifrost's job. The
game makes **one request per action** and never retries in a loop; per-request `fallbacks`
(a list of model strings in the request body) are the only fallback mechanism, and they are
configuration, not code.
*Pinned by:* `Cargo.toml` review (no `aws-sdk-*`, no `tokio` in the default or `ai` feature
set) and `ai::tests::one_request_per_action`.

**R6 — No secrets in the game.** AWS keys, regions, role ARNs and inference-profile ARNs live in
Bifrost's config, referenced there as `env.AWS_ACCESS_KEY_ID` and friends. The game's config
holds a base URL, an optional bearer token for the gateway itself, model strings and timeouts.
Nothing in `ui.toml` is a cloud credential.
*Pinned by:* `ui::config::tests::ai_config_has_no_credential_fields` (the serialized struct's
field names are checked against a deny list).

**R7 — Never block the frame loop or the step.** All gateway calls run on the worker. A screen
enqueues a request tagged with a **request id** and the feature id, and reads replies from a
channel on later frames. Every request carries a timeout (`[ai] timeout_secs`, default 30).
Stale replies — a reply whose request id the screen no longer waits on — are dropped. Turning
the master switch off in Options stops the worker and discards in-flight replies.
*Pinned by:* `ai::tests::stale_reply_is_dropped`, `ai::tests::timeout_yields_error_reply`, and
the UI test in R2.

**R8 — Model output is data.** Text from the model is displayed, or parsed and validated, and
never executed, `include!`d, `eval`'d or passed to a shell. Upstream output is parsed with the
same strict loader the hand-written file uses (`Params::validate`, TOML deep-merge rules) and a
validation failure is shown to the player and, where the feature allows it, sent back to the
model for one more attempt at most. Structured output is requested with a JSON schema or a
tool definition; the game never regex-scrapes prose for values. Model text drawn on screen is
passed through the CP437 filter (`glyphs` helpers) so a non-CP437 character becomes `?` rather
than a wide cell.
*Pinned by:* `ai::tests::non_cp437_output_is_sanitised` and
`ai::tests::designer_rejects_invalid_overlay`.

**R9 — Say what leaves the machine.** Each feature's spec lists exactly what it sends: which
fields, how many events, whether the full parameter set. The Options screen shows the model
string beside each feature so the `bedrock/` prefix reads as cloud at a glance, and the
`--dump-params` comment for each `[ai.features]` key repeats the data list. Never sent: save
files, file-system paths, the player's config, environment variables.
*Pinned by:* `params::tests::field_docs_complete` (every `[ai]` key has a `FIELD_DOCS` entry)
and review of the spec's *Sends* line.

**R10 — Headless is opt in per invocation.** A headless run calls the gateway only when the
command line names an AI flag (`--chronicle`, `--postmortem`, `--design-species`, …). The
`[ai]` table supplies the URL and model but never switches a headless feature on. An AI flag
with no reachable gateway exits non-zero with one line naming the URL; it never hangs, and it
never silently falls back, because batch output must be what the flag promised or nothing.
`--seeds`, `--summary`, `--row`, `--profile`, `--header` and the sweep script never touch the
network under any configuration.
*Pinned by:* `tests/headless.rs::ai_flags_absent_runs_are_offline` and
`tests/headless.rs::ai_flag_without_gateway_exits_nonzero`.

**R11 — Saves stay loadable without AI.** Anything AI-generated that is stored in a save is
decorative and optional: a region-name table, a chronicle text, a creature-name table. The
format bump that adds one makes the table optional in the reader, and a save written with the
table loads in a build without the `ai` feature and in a run with AI off. Nothing the UI or the
sim needs in order to function is AI-generated.
*Pinned by:* `sim::save::tests::ai_tables_are_optional_on_load`.

**R12 — Tests are offline.** No test tier contacts a real gateway. Acceptance and unit tests use
a **fake gateway** *(C9: `scripts/fake-gateway.js`, a Node `node:http` script the tests launch
on an ephemeral port through `ai::fake::Fake`, rather than an in-process Rust listener; it
doubles as the development mock)* that returns canned completions from
`tests/fixtures/ai/<feature>/<scenario>.json` and records every request it receives. The fake is
the only place the `ai` feature's tests run, so `just test-unit-ai ai` and the affected chunk
stay within their tiers. The AI tests need Node on `PATH`.
*Pinned by:* `scripts/hooks/guard-cargo-test.sh` unchanged; `ai::tests::*`, `ui::tests::ai::*`
and `tests/headless.rs::with_gateway::*` construct the fake gateway in their setup.

**R13 — Existing invariants apply unchanged.** CP437 glyphs only, colours from `theme.rs`, the
800-line ceiling, the strict clippy set, `cast!` for every numeric cast, parameters with
`FIELD_DOCS`, and the key-routing rules for screens. An AI feature is an ordinary feature that
happens to make an HTTP request.
*Pinned by:* `just check` and `tests/file_size.rs`.

**R14 — Compile-time optional.** The HTTP dependency and the network half of `src/ai/` sit
behind an `ai` Cargo feature that is **not** in `default` *(C9: the plain types, the prompt
builders, the CP437 filter and the headless stubs compile always, so screens and `main.rs`
carry no `cfg`; `Ai::start` is `Off` and the AI flags say "built without the ai feature")*. `cargo build` with defaults produces today's
binary with no new dependency; CI and contributors need nothing. With the feature compiled in,
R1 still governs at run time.
*Pinned by:* CI building both `--no-default-features` (plus whatever defaults exist) and
`--features ai`; `just check` runs clippy under both.

## Configuration

The `[ai]` table lives in `ui.toml` beside the other UI options (`~/.config/sim-fortress/ui.toml`
or `$XDG_CONFIG_HOME/sim-fortress/ui.toml`), and is edited from the Options screen. Params
overlays (`params.toml`, `--params`) do **not** carry it: AI is a property of the player's
machine, not of a world.

```toml
[ai]
enabled = false                        # master switch; default false; nothing runs while false
base_url = "http://localhost:8080/v1"  # Bifrost's OpenAI-compatible root
token = ""                             # optional bearer token for the gateway itself (never a cloud key)
timeout_secs = 30                      # per request; a timeout is an error reply, never a retry

[ai.features]
# A feature is on only when it names a model. Absent, empty or false = off.
# Model strings are Bifrost's "<provider>/<model>"; the prefix tells you where the data goes.
chronicle = "ollama/llama3.1:8b"
chronicle_fallbacks = ["bedrock/claude-sonnet"]   # tried by Bifrost, in order, if the first fails
designer = "bedrock/claude-sonnet"
designer_fallbacks = []
# Later features add their key here the same way: names, bio, thoughts, postmortem, ...
```

*(C9)* Model keys are plain strings and `""` means off; the `bio = false` form is not accepted
because `AiConfig` also travels inside `Params` in binary saves, and postcard cannot read an
untagged bool-or-string. `AiConfig` is a field of `UiParams`, so `--dump-params` lists these
keys as `ui.ai.*` while `ui.toml` (which is `UiParams` at its root) shows `[ai]`. The handle
is built from `ui::config::load_ai()` only, never from `params.ui.ai`. The `token` newtype
writes `""` to non-human-readable formats so a save never carries it.

Rules for the table:

- Every key has a `FIELD_DOCS` entry, and each `[ai.features]` entry's comment states what the
  feature sends (R9).
- Adding a feature adds a key here, an Options row, a help line, and a fixture directory. It
  never adds a second table or a second config file.
- The Options section is one master checkbox, then one row per feature (model string, greyed
  until the master is on). Toggling the master off stops the worker immediately (R7).
- Options shows a status cell: `off`, `offline` (enabled, probe failed), or `ready`. The probe is
  `GET /v1/models` on the gateway, made **only** after the master is on, and re-tried every
  60 s while `offline`.

## Architecture

```
src/ai/                 behind `--features ai`; never imported from src/sim
  mod.rs                pub API: AiConfig (re-export), Ai handle, Request, Reply, Status
  client.rs             one blocking HTTP client; chat_completion(model, messages, schema?, fallbacks?)
  worker.rs             the thread; channel in, channel out; request ids; timeouts; probe
  prompt/               one file per feature: builds the messages + schema from sim data
    chronicle.rs, names.rs, designer.rs, ...
  sanitize.rs           CP437 filter for on-screen text (R8)
  fake.rs               #[cfg(test)] fake gateway used by every test (R12)
src/ui/screens/…        a screen enqueues via Ai::request(feature, payload) and polls Ai::poll()
src/main.rs             headless AI flags construct the same Ai handle synchronously
```

- **One handle.** `Ai` is constructed once by `App` (live) or `main` (headless) from
  `AiConfig`. When `enabled` is false it is `Ai::Off`, a unit variant whose `request` is a
  no-op returning `Err(AiError::Off)`, so screens need no `if let Some(ai)` sprawl.
- **Requests and replies are plain enums.** `Request { id, feature, payload }` and
  `Reply { id, feature, result: Result<Output, AiError> }`. `Output` is `Text(String)`,
  `Json(serde_json::Value)`, or `Chunk(String)` for streaming, followed by `Done`. Screens
  match on the feature id they own and ignore the rest.
- **The worker is the only thread.** One `std::thread` with `std::sync::mpsc` both ways. No
  async runtime: the HTTP client is blocking and small. Streaming (server-sent events) is read
  on the worker and forwarded as `Chunk` replies, so the chronicle can draw as it arrives.
- **Prompts live in `src/ai/prompt/`,** one module per feature, and take `&Sim` or a
  pre-extracted plain struct, never `&mut`. They are unit-tested for *what they include* — the
  R9 data list — by asserting on the built message, not by calling a model.
- **Structured output.** Upstream features send a JSON schema (`response_format`) and, if the
  gateway or backend rejects it, fall back to a single tool definition whose parameters are
  the schema. Which of the two Bifrost passes through to Ollama and to Bedrock is an open
  item below; the code supports both so the answer only changes a default.
- **States.** `Status::{Off, Offline, Ready}`, derived by the worker from the probe. The status
  bar draws nothing for `Off`, a dim `AI: offline` for `Offline`, nothing for `Ready` (the
  feature key hints are the indicator).

## Bifrost setup the game assumes

The game assumes nothing about which backends exist. This is the reference setup the docs and
fixtures use; the player runs it with `npx -y @maximhq/bifrost` or the Docker image, on port
8080, and configures it through Bifrost's own UI or `config.json`:

```json
{
  "providers": {
    "ollama": {
      "keys": [{ "name": "local", "value": "dummy", "models": ["*"], "weight": 1.0 }],
      "network_config": { "base_url": "http://localhost:11434", "allow_private_network": true }
    },
    "bedrock": {
      "keys": [{
        "name": "aws", "models": ["*"], "weight": 1.0,
        "aliases": { "claude-sonnet": "<your region's Sonnet model id>",
                     "claude-opus": "<your region's Opus model id>" },
        "bedrock_key_config": {
          "access_key": "env.AWS_ACCESS_KEY_ID",
          "secret_key": "env.AWS_SECRET_ACCESS_KEY",
          "region": "us-east-1"
        }
      }]
    }
  }
}
```

Omit `access_key` and `secret_key` to use the normal AWS credential chain. Bifrost's semantic
cache is welcome for `names` and `thoughts` (repeated, near-identical prompts) and must be off
for `chronicle` and `postmortem` (a cache hit would repeat last season's story); how that is
scoped is an open item below.

## How to build an AI feature

1. **Write the spec** using the template below, in the chunk doc that ships the feature. Name
   the fallback first. If you cannot name an existing behaviour as the fallback, the feature
   is not ready.
2. **Add the config key** to `[ai.features]` with its `FIELD_DOCS` comment stating what is
   sent. Add the Options row and the S11 help line.
3. **Write the prompt module** in `src/ai/prompt/<feature>.rs`: a function from plain sim data
   to messages plus an optional schema. Unit-test that the message contains exactly the R9
   data list and nothing else.
4. **Wire the request** from the screen or the headless flag through the `Ai` handle. Screens
   draw the fallback until a reply arrives and after any error. Headless flags exit non-zero on
   error.
5. **Validate the output.** Downstream: sanitise for CP437 before drawing. Upstream: parse with
   the existing loader, run `Params::validate`, show errors, allow at most one automatic
   correction round.
6. **Record fixtures** under `tests/fixtures/ai/<feature>/` for the happy path, a malformed
   reply, and a timeout. Add the tests named in R1–R12 that the feature touches.
7. **Update the docs**: the chunk doc's status, the screen requirement (`docs/screens/`) and its
   render snapshot with AI **off** (the snapshot set stays AI-free), and `feature-ideas.md`
   (strike the idea, note what shipped).
8. **Run** `just check` under both feature sets, `just test-unit ai`, the affected chunk, and
   the checksum test.

## Feature spec template

```
### <Feature name>   (`[ai.features] <id>`)
Direction: upstream | downstream
Fallback: <the existing behaviour with AI off, by screen or flag name>
Trigger:  <key on which screen, or headless flag>
Sends:    <exact list: fields, counts, whether params are included>
Receives: <text | JSON matching schema <name> | stream>
Stores:   nothing | report file <name> | optional save table <name> (R11)
Model:    <suggested default model string and why; cache: yes | no>
Failure:  <what the player sees on timeout / error / invalid output>
Tests:    <fixture names; which R-tests this feature adds cases to>
```

## Failure semantics

| Situation | Live app | Headless |
|---|---|---|
| `[ai]` absent or `enabled = false` | Today's game; no worker, no probe, no hints | AI flags error: "AI is not enabled in ui.toml" |
| Enabled, gateway unreachable | `AI: offline` in status; features draw fallback; probe every 60 s | AI flag exits non-zero naming the URL |
| Enabled, feature has no model | Feature absent from screens | AI flag exits non-zero naming the key |
| Request timeout or HTTP error | One dim notice line, then fallback; no retry | Exit non-zero with the gateway's message |
| Reply invalid (bad JSON, fails `validate`) | Notice + fallback; upstream features may retry once with the error | Exit non-zero; upstream features retry once |
| Reply for a stale request id | Dropped silently | n/a |
| Master switched off mid-request | Worker stopped; reply discarded | n/a |

## Open items to settle against a running gateway

These do not block the design; each only changes a default or a doc line.

- Whether `response_format` with a JSON schema passes through Bifrost to Ollama and to Bedrock,
  or whether the tool-definition form is needed for one of them.
- Whether Bifrost's semantic cache can be scoped per model alias or per header, so `names` is
  cached and `chronicle` is not, without two gateway instances.
- Whether the `/v1/models` listing includes Bedrock aliases, so the Options `ready` probe can
  also flag a feature whose model string is unknown to the gateway.
- Whether Bifrost's MCP support can host the read-only sim API for *ask the world*, or the
  game needs its own tool loop.
