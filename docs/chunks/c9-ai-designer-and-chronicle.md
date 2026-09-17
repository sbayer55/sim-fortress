# C9 — AI species designer and chronicle narrator

Back to the [roadmap](README.md). Previous: [C8](c8-sociality-and-maturity.md). Bound by
[ai-requirements.md](../ai-requirements.md) (R1–R14), which this document references rather
than restates.

> **Status: implemented (2026-09-16), tested against a fake gateway only.** Both features,
> the shared plumbing (`[ai]` config, worker thread, blocking client, Node fake, headless
> flags, Options section) and every test named in R1–R12 are in the tree and green under
> both feature sets. Nothing has yet been run against a real Bifrost instance, so the two
> open items in the requirements doc (`response_format` pass-through, cache scoping) remain
> open; the demo script's last step is that check.

## Goal
Let a language model *write inputs the sim already accepts* and *read outputs the sim already
produces*, without the simulation ever knowing a model exists. The species designer turns a
sentence into a `[[species]]` overlay that goes through the same loader as a hand-written
file. The chronicle narrator turns each season's slice of the event ring plus the census into
a legends-style paragraph. One upstream and one downstream feature together force every piece
of plumbing later AI ideas reuse.

## Checkpoint (what the user sees)
With no `ui.toml` the game is byte-for-byte what it was, except that the event log now has a
`c` key: a **chronicle** view that, at each season boundary, gains a deterministic tally
entry ("Voles 240→198, … 412 births, 455 deaths (predation 210, …). The Wolves are
extinct."). With `[ai] enabled = true` and a model named for `chronicle`, that tally is
replaced a few seconds later by a paragraph streamed from the model, marked `chronicled`
instead of `tally`. With a model named for `designer`, the New World form grows a fourth
button, `[ Design species ]`, that opens a one-line prompt; the reply is validated by the
roster loader (one correction round at most), previewed, and on Enter appended to the roster
and written to `~/.config/sim-fortress/species/<name>.toml`. Options (`p`) shows an **AI**
section: master switch `[m]`, status `off | offline | ready | not compiled`, and the model
string beside each feature. An unreachable gateway shows a dim `AI: offline` on the map's
status bar and nothing else changes. Headless, `--chronicle` writes `chronicle.md` and
`--design-species "…"` writes an overlay file; both exit 1 with one line if AI is off,
unreachable or not compiled.

## Scope

### In
- The `[ai]` table (`AiConfig` on `UiParams`, `src/sim/params/ai.rs`), persisted in
  `ui.toml`, documented in `FIELD_DOCS` as `ui.ai.*`, read only through `config::load_ai`.
- `src/ai/`: plain types (`Ai`, `Request`, `Reply`, `Output`, `Status`, `AiError`,
  `Feature`), the CP437 sanitiser and the prompt builders always compiled; the blocking
  `ureq` client, the worker thread, the headless bodies and the fake launcher behind
  `--features ai` (not in `default`).
- The chronicle: `src/sim/chronicle.rs` (season slice, season census, template entry),
  `Sim.chronicle` in the save (VERSION 11), the season trigger in `AppState::step_ticks`, the
  reply pump in `src/ui/ai_bridge.rs`, S07c, `--chronicle`.
- The designer: `src/ai/prompt/designer.rs` (schema, messages, JSON→TOML, `validated`,
  summary), S09b (`src/ui/screens/s09_worldgen/designer.rs`), the `[ Design species ]`
  button and `WorldGenForm::apply_species_overlay`, `--design-species`.
- S10 AI section and `m`; `AI: offline` in the S01 status bar; S11 lines.
- `scripts/fake-gateway.js` (Node, `node:http` only) and `tests/fixtures/ai/`.

### Out
- Editing model strings from the TUI (they are set in `ui.toml`; S10 shows them).
- Every other AI idea in [feature-ideas.md](../feature-ideas.md): named places, bios,
  ask-the-world, post-mortem, preset-from-prompt, field guide, thoughts, balance agent.
- Streaming for the designer (it needs the whole JSON), and any retry beyond the single
  correction round.
- Running the chronicle for seasons that ended before AI was switched on.

## Dependencies
- C6 `ui.toml` persistence and the S10 Options section; C6 `--params` overlays and
  `Params::apply_overlay` (deep-merge by name, `validate`); C6 save versioning.
- C2 event ring and season events; C4 `SpeciesStats`/`Series` for the census.
- Node on `PATH` for the AI tests (`just test-unit-ai`, `cargo test --features ai --test headless`).
- `ureq 3` and `serde_json`, optional, pulled in only by `--features ai`.

## Model overview
```
ui.toml [ai] ──load_ai()──► Ai::start ──► worker thread (ureq, probe every 60 s while offline)
                                 ▲                 │ Reply{id, feature, Chunk|Text|Done|Err}
   S07c / S09b / S10 ── request ─┘                 ▼
                              AppState::pump_ai ── chronicle entry / designer inbox
   Sim.step ──season boundary──► template_entry ──► Sim.chronicle (decorative, saved)
   designer reply (JSON) ──► [[species]] TOML ──► Params::apply_overlay ──► form roster + file
```
The step never reads `Sim.chronicle`, `AiConfig` or anything under `src/ai` (R3): the
checksum is unchanged with the feature on and off.

## Functional requirements

### FR1 Config (`[ai]` in `ui.toml`)
```toml
[ai]
enabled = false                        # master switch; nothing runs, probes or listens while false
base_url = "http://localhost:8080/v1"  # Bifrost's OpenAI-compatible root
token = ""                             # optional bearer token for the gateway itself; blank in saves
timeout_secs = 30                      # per request; a timeout is an error reply, never a retry

[ai.features]
chronicle = ""                         # "<provider>/<model>" or "" = off
chronicle_fallbacks = []               # tried by Bifrost, in order
designer = ""
designer_fallbacks = []
```
- `AiConfig` is plain data on `UiParams`, so it serialises at the root of `ui.toml` and
  appears in `--dump-params` under `ui.ai.*` (the prefix differs from the file, which is
  `UiParams` at its root). `token` is a newtype that writes `""` to non-human-readable
  formats, so a save never carries it.
- The handle is built from `config::load_ai()` only. A `--params` overlay with `[ui.ai]`
  parses (the struct is in `Params`) but never switches AI on, in the live app or headless.
- Model keys are strings, `""` = off; postcard cannot read an untagged bool-or-string.

### FR2 The handle, the worker and the client (`src/ai/`)
- `Ai::start(&AiConfig)` is `Ai::Off` unless `enabled` and the feature is compiled in.
  `request(feature, messages, schema, stream) -> RequestId`, `poll() -> Vec<Reply>`,
  `cancel(id)`, `wait(id)` (headless only), `status()`, `feature_on(feature)`,
  `status_note()` (`"AI: offline"` for `Offline`, `""` otherwise).
- One `std::thread` with `mpsc` both ways. `GET /v1/models` on start and every 60 s while
  offline. One HTTP request per action; `fallbacks` in the body is the only fallback (R5).
  A reply for a cancelled id is dropped in `poll` (R7). Dropping the handle closes the
  channel; the thread exits after its current call and the reply dies with the receiver.
- The client sets `x-simf-feature: <key>` on every chat request, uses `response_format`
  `json_schema` when a schema is given, streams SSE as `Chunk`s, and turns a non-2xx body's
  `error.message` into `AiError::Gateway`. Connection failures are `Unreachable(url)`.
- `sanitize::to_cp437` folds typographic punctuation to ASCII and turns anything outside
  CP437 into `?` before text is drawn (R8).

### FR3 Chronicle data (`src/sim/chronicle.rs`, pure)
- `ChronicleEntry { year, season, text, source: Template | Model }`; `Sim.chronicle`.
- `season_events(sim, year, season)` filters the ring by `(year, (day-1)/season_days)`;
  `season_census` gives per-species start (first series sample of the season), end
  (`SpeciesStats.count`), births, deaths and the extinct flag; `template_entry` writes the
  tally line, the death causes and up to eight notable events (extinction, epidemic,
  outbreak, spillover, drought, migration, note).
- Save VERSION 10 → 11 carries the field; an empty table loads in any build (R11).

### FR4 Chronicle in the live app (S07c)
- `AppState::step_ticks` compares `time.season()` before and after a batch (≤ 200 ticks
  against ≥ 720 per season, so one boundary at most) and records the `(year, season)` that
  ended. `enqueue_chronicle` (every frame) pushes the template entry and, if
  `feature_on(Chronicle)`, one streamed request tagged with the entry index, cancelling any
  older one. `pump_ai` appends chunks to the entry as they arrive, marks it `Model` on
  `Done`, and on error restores the template and sets `ai_notice`.
- S07 `c` toggles the chronicle view: newest first, `Year N, Season` headers with the season
  glyph, wrapped text, `tally | chronicled | writing...` tags, `↑↓` scroll, the notice line
  at the top when set. The `c` hint is always present (the view exists with AI off).

### FR5 Chronicle headless (`--chronicle [--out FILE]`)
- Steps the run, and at every season boundary makes one blocking request (template first,
  so the Markdown carries the tally in italics under the paragraph). Writes `chronicle.md`
  in the cwd or `--out`. Fails before stepping if AI is off, the key names no model, or the
  probe fails; fails on the first request error. Never falls back (R10).

### FR6 Species designer (`src/ai/prompt/designer.rs`, S09b, `--design-species`)
- Prompt: system message with the rules, the `species.*` field docs, one example entry
  and the roster as `(name, kind, glyph)`; user message with the sentence, plus the
  rejected reply and the loader's error on the one correction round.
- Reply (JSON, `SCHEMA`) → `toml::Value` → `[[species]]` text → `validated(base, toml)` =
  `apply_overlay` on a clone. Deep-merge by name means a new name appends and a known name
  edits in place; `deny_unknown_fields` and `validate_species` catch the rest.
- S09b modal phases: `Typing → Waiting → Preview | Failed`. Preview shows the summary lines
  (name/plural/glyph/kind/diet, life history, non-average genome traits, prey shares);
  Enter writes `<config dir>/species/<name>.toml` and hands the overlay to the form through
  `AppState.pending_species_overlay`; S09 applies it at its next key with
  `apply_species_overlay` (roster and counts replaced, focus clamped, preview untouched).
- The `[ Design species ]` button is the last Tab stop and exists only while the feature is
  on; the other three buttons are unchanged except `[ Randomize seed ]` → `[ Randomize ]`
  so all four fit the 64-column row.
- Headless: one request, one correction round, `species-<name>.toml` in the cwd or `--out`,
  summary lines on stdout, exit 1 otherwise.

### FR7 Options, status, help
- S10 gains an **AI** section (modal 60×26): `[ ] AI enabled   status: …   [m]`, then one
  row per feature with its model string or `(off)`, dim until the master is on. `m` flips
  `enabled`, persists `ui.toml`, drops the old handle and starts a new one from
  `config::load_ai()`.
- S01's status bar draws `AI: offline` in dim text left of the clock only for
  `Status::Offline`; `Off` and `Ready` draw nothing, so the S01a snapshot is unchanged.
- S11 Screens group: `e  event log (c: chronicle)`, `p  controls, options, AI`.

### FR8 Feature specs (template from ai-requirements.md)
```
### Chronicle   (`[ai.features] chronicle`)
Direction: downstream
Fallback: the template entry in S07c; `--chronicle` has none and exits 1
Trigger:  automatic at each season boundary while enabled; `c` in S07 views it
Sends:    year, season, region names, that season's events as (day, label, text) capped
          at 200 with non-birth/death kinds first, per-species (plural, start, end, births,
          deaths, extinct), the previous entry's text. Never params, paths or config.
Receives: streamed text
Stores:   save table Sim.chronicle (R11); chronicle.md headless
Model:    ollama/llama3.1:8b; cache: no
Failure:  template stays; one dim notice line in S07c; headless exits 1
Tests:    fixtures chronicle/{happy,malformed,timeout,error}; ai::tests::gateway::*,
          ui::tests::ai::chronicle_streams_in_from_the_fake, headless chronicle_writes_markdown_against_fake

### Species designer   (`[ai.features] designer`)
Direction: upstream
Fallback: hand-write the overlay and pass it with --params
Trigger:  [ Design species ] on S09; --design-species PROMPT
Sends:    the sentence, the species.* field docs, one example [[species]] entry, the
          roster's names/kinds/glyphs; on retry the rejected reply and the loader's error
Receives: JSON matching SCHEMA (response_format json_schema)
Stores:   <config dir>/species/<name>.toml (live); species-<name>.toml or --out (headless)
Model:    bedrock/claude-sonnet; cache: no
Failure:  the loader's message in the modal, form unchanged; headless exits 1
Tests:    fixtures designer/{happy,invalid_then_valid,malformed,timeout};
          ai::tests::designer_rejects_invalid_overlay, ui::tests::ai::designer_*,
          headless design_species_*
```

## Acceptance criteria
- **Off is off (R1).** With no `[ai]` table, or `enabled = true` and no model named, two
  seasons and every screen produce zero gateway requests. *pass*
  (`ui::tests::ai::ai_off_makes_no_requests`, `ai_master_on_without_features_makes_no_requests`,
  `tests/headless.rs::with_gateway::ai_flags_absent_runs_are_offline`).
- **Unreachable is harmless (R2).** Enabled with a closed port: the app steps, the chronicle
  keeps its template entries, the status bar reads `AI: offline`. *pass*
  (`ui::tests::ai::ai_unreachable_reaches_map_and_steps`).
- **The step never sees the model (R3).** `sim::tests::checksum_is_fnv_stable` is
  `0x9b4ed39b8baac6f5` with and without `--features ai`; `src/sim` has no AI code. *pass*.
- **One request per action, no retries (R5), stale replies dropped and timeouts are errors
  (R7).** *pass* (`ai::tests::gateway::*`).
- **Model output is data (R8).** Invalid overlays are rejected by the real loader; exactly
  one correction round; non-CP437 text is sanitised. *pass*
  (`ai::tests::designer_rejects_invalid_overlay`, `designer_retries_once_on_invalid_overlay`,
  `design_species_retries_once`, `non_cp437_output_is_sanitised`).
- **Headless is opt in and honest (R10).** AI flags exit 1 with one line when off,
  unreachable or not compiled; `--seeds/--summary/--row/--header` never touch the network.
  *pass* (`tests/headless.rs`).
- **Saves stay loadable (R11).** *pass* (`sim::save::tests::ai_tables_are_optional_on_load`).
- **Against a real Bifrost.** Not run; see the demo script's last step.

## Checkpoint demo script
1. `cargo run --release` with no `ui.toml`: press `e`, then `c` — an empty chronicle with the
   note that the first entry is written when the season turns. Speed up to x25; when Summer
   arrives, `c` shows `Year 1, Spring   tally` with the counts. `p` shows the AI section with
   `[ ] AI enabled   status: off`.
2. `node scripts/fake-gateway.js --port 8089` in another terminal. Write
   `~/.config/sim-fortress/ui.toml`:
   ```toml
   [ai]
   enabled = true
   base_url = "http://localhost:8089/v1"
   [ai.features]
   chronicle = "fake/m"
   designer = "fake/m"
   ```
   `cargo run --release --features ai`: `p` reads `status: ready`. Let a season pass; in S07c
   the entry is tagged `writing...` then `chronicled`, and the text mentions the quiet valley.
3. Stop the fake. Within a minute `AI: offline` appears in the map's status bar; the next
   season's entry stays a `tally` and S07c shows the notice line. Restart the fake; it clears.
4. Title → New World: the fourth button `[ Design species ]` is present. Enter, type
   `a boar`, Enter: `validated: boar` with the summary lines; Enter again: the species rows
   now end with `boar 30` and `~/.config/sim-fortress/species/boar.toml` exists.
5. `cargo run --release --features ai -- --headless --seed 1 --years 1 --chronicle` writes
   `chronicle.md` with four `## Year 1, …` sections.
   `cargo run --release --features ai -- --design-species "a boar: omnivore, big litters"`
   writes `species-boar.toml`; `--headless --years 1 --summary --params species-boar.toml`
   accepts it and the CSV gains `boar` columns.
6. `cargo run --release -- --headless --seed 1 --ticks 24 --chronicle` (no feature) exits 1
   with `built without the ai feature`.
7. *Real gateway (not yet done):* point `base_url` at Bifrost on 8080 with Ollama behind it
   and repeat steps 2 and 4; record whether `response_format` reached the model or the tool
   form is needed, and update the open items in ai-requirements.md.

## Tests
- `src/ai/tests.rs`: `non_cp437_output_is_sanitised`, `off_handle_is_inert`,
  `designer_rejects_invalid_overlay`, and under the feature `gateway::{probe_marks_ready_then_offline,
  one_request_per_action, streaming_delivers_chunks_then_done, stale_reply_is_dropped,
  timeout_yields_error_reply, malformed_reply_is_invalid}`.
- `src/ai/prompt/{chronicle,designer}.rs`: the R9 "sends exactly" tests.
- `src/sim/chronicle.rs`, `src/sim/save.rs::ai_tables_are_optional_on_load`,
  `src/ui/config.rs::{ai_config_has_no_credential_fields, gateway_token_blank_in_binary_saves}`.
- `src/ui/tests.rs`: `chronicle_template_pushed_at_season_boundary`; under the feature
  `ai::{ai_off_makes_no_requests, ai_master_on_without_features_makes_no_requests,
  ai_unreachable_reaches_map_and_steps, chronicle_streams_in_from_the_fake,
  designer_modal_round_trip, designer_retries_once_on_invalid_overlay}`.
- `src/ui/screens/s09_worldgen/tests.rs`, `s10_controls::tests::s10_ai_master_toggle`.
- `tests/headless.rs`: `ai_flag_without_gateway_exits_nonzero` (both builds) and, under the
  feature, `with_gateway::{ai_flags_absent_runs_are_offline, chronicle_writes_markdown_against_fake,
  design_species_writes_overlay_against_fake, design_species_retries_once}` — the first
  tests in the repository that spawn the binary.
- Run: `just check` (clippy under both feature sets), `just test-unit-ai ai`,
  `just test-unit-ai ui`, `cargo test --features ai --test headless`. The AI tests need
  Node on `PATH`; `Fake::start` says so if it is missing.

## Implementation order (as built)
1. `AiConfig` on `UiParams`, FIELD_DOCS, `config::load_ai`, Cargo feature, justfile and
   `affected-tests.sh` mapping.
2. `src/ai` types, client, worker, sanitiser; `scripts/fake-gateway.js`; fixtures; tests.
3. `src/sim/chronicle.rs`, `Sim.chronicle`, save VERSION 11.
4. Chronicle prompt, `ai_bridge`, `step_ticks` trigger, S07c.
5. Designer prompt and schema, S09b modal, form hand-back, `common::edit_text`.
6. S10 AI section and `m`, `status::render_noted`, S11.
7. Headless bodies, `main.rs` flags, subprocess tests.
8. This document, the roadmap, screen docs, renders, requirement amendments.

## Decisions made here
- **`AiConfig` lives on `UiParams`,** not in a second file: `ui.toml` is already the
  per-machine config and `FIELD_DOCS` covers it for free. The cost is that the struct also
  travels in `Params` and hence in saves; the token newtype blanks itself there.
- **The handle is built from `ui.toml` only,** never from `params.ui.ai`, so overlays cannot
  enable AI even when `ui.toml` is absent.
- **Model keys are strings, `""` = off.** The doc's `bio = false` form would need an untagged
  enum, which postcard cannot read.
- **Plain types, prompts and the sanitiser compile always;** only the network code is
  feature-gated. Screens and `AppState` therefore carry no `cfg`, and the headless flags are
  recognised in both builds so the stub can say "built without the ai feature".
- **The fake gateway is a Node script**, launched by the tests, rather than an in-process
  Rust listener: it doubles as the development mock and keeps HTTP parsing out of the crate.
- **The template chronicle always exists.** The `c` key and the view are ordinary features;
  the model only replaces the text.
- **Season boundaries are detected in `step_ticks`,** by comparing seasons around a batch,
  so `StepReport` in `src/sim` stays untouched.

## Risks
- `timeout_global` bounds the whole streamed chronicle call; a slow local model on a long
  season may need a larger `timeout_secs`.
- The gateway's `json_schema` support is unverified against Bifrost/Ollama/Bedrock; the
  prompt also states the shape in words, and the loader is the real gate, so a backend that
  ignores the schema degrades to the correction round rather than to garbage.
- The ring holds 5 000 events; a very busy season can lose its earliest events before the
  boundary, so the tally's births/deaths are lower bounds. The census start/end counts come
  from the series and are exact.
- `XDG_CONFIG_HOME` is process-global; tests that set it hold `config::env_lock()`.
