---
name: tui-mockup
description: Build a throwaway, keyboard-driven web mockup of a Sim Fortress terminal screen with several variants side by side, then verify it in the browser pane and hand the chosen variant off as a screen spec and implementation plan. Use this whenever the user asks to mock up, sketch, prototype, explore, compare or "see what it could look like" for any TUI screen, modal, sidebar, status bar, switcher, picker or overlay in this repo, even if they do not say "mockup" or "variants", and also when they later say "add a variant", "combine 2 and 3", "make an animated version", or "write the requirements for variant N".
---

# TUI mockup with variants

A mockup is cheaper than a redesign argument. The point of this skill is to get three
or more genuinely different answers to the same design question in front of the user
within one session, on a strict 155×45 character grid with the real theme colours, so
the one they pick can be specified cell for cell and built without surprises. The
S14 Overlay Switcher (Sept 2026) went mockup → six variants → spec → plan → shipped
this way; `docs/screens/s14-overlay-switcher.md` shows where it ends up.

Nothing here touches the repo's code. The mockup lives in the scratchpad; only the
spec and plan (step 6) become commits.

## 1. Ask first, then read the code

Ask three or four questions with `AskUserQuestion` before building anything, because
each answer changes what the variants should differ *in*. Good axes, seen in practice:

- **Placement**: sidebar list, popup over the map, horizontal strip, status-bar flyout.
- **Nesting**: are second-level choices (which species, which pathogen) inside the
  widget, outside it, or should the variants split on this?
- **Exclusivity**: one thing active, or stacking; should one variant explore stacking?
- **Fidelity**: strict terminal grid (default here) or a looser web sketch.

Offer "show both approaches" as an option on the nesting and exclusivity questions;
users often want one variant each way. Skip the questions only when the request
already answers them ("assume …" in the prompt).

Then read the real sources so the mockup is not a guess. In one pass:

| Need | Where |
|------|-------|
| Colours | `src/theme.rs` (every `Color::Rgb`), ramps `heat`/`veg`/`water`/`parasite` |
| Glyphs | `src/glyphs.rs`; every glyph must be CP437 and one cell wide |
| Frame and split | `src/ui/viewport.rs` (`SIDEBAR_W` 43, `GUTTER_W`, `MAP_CHROME_ROWS`), 155×45 |
| The screen being changed | `src/ui/screens/sNN_*.rs` and its `docs/screens/sNN-*.md` |
| A precedent for the widget kind | `docs/components/*.md` (modal, menu, checkbox, table, filter-strip) and the screen that uses it |
| Real names for fake data | `src/sim/params/species.rs` roster, `src/sim/params/pathogen.rs`, region names in `src/sim/world/` |
| Key conventions | `docs/components/keys.md`; the status bar must advertise every handled key |

The template already carries the theme table, the default roster and pathogens, but
they drift: grep the file before trusting a value.

## 2. Build from the template

Copy `assets/mockup-template.html` into the scratchpad under a descriptive name and
fill in `VARIANTS`. The template gives you:

- a 155×45 cell buffer with `put/text/fill/box/section/rampRow` mirroring the Rust
  widgets (outer `═`, inner `─`, focus border in the focus colour), plus `withClip`
  and `FADE` for motion;
- the S01 chrome: a seeded fake world in the 112×42 map panel, the 43-column status
  sidebar, the ticker row and a `StatusBar`-style status row; `modal(area,w,h,…)`
  centres a focus panel on any rect; `dimAll()` is the 0.55 backdrop dim;
- a variant registry: each variant is `{id, name, blurb, st, draw(), keys(k,e)}`,
  gets a tab, and owns its state, so variants never leak into each other;
- key normalisation (Space and Enter arrive the same from every browser), a
  fit-to-width scale, a 20 fps tween engine with a 60 fps toggle, and `dump()` for
  cell-exact text.

Rules that make the result trustworthy:

- **Every variant answers the same questions.** Same feature, same data, same keys
  where the design does not differ; the tabs must compare like for like.
- **Each cell is one glyph, one fg, one bg, optional bold.** No CSS shadows, no
  fractional positions, nothing a `ratatui` `Buffer` cannot hold. If the fidelity
  answer was "loose sketch", relax this deliberately and say so in the blurb.
- **Only CP437 glyphs, and check it, do not assume it.** The code is held to
  `glyphs::CP437` by a test, so a mockup glyph outside that set is a promise the build
  cannot keep. The template carries the same set: `put()` warns in the console the
  first time a stray glyph is drawn, and `checkGlyphs()` returns every offender in the
  current buffer with a position. Run `checkGlyphs()` for every variant in every state
  (modal open, list focused, each tab) and fix the glyph, not the check. The usual
  offenders and their CP437 stand-ins: `●` → `■` or `•`, `—` → `─` or `-`, `…` → `·`,
  `›` → `»`, `✓ ✗` → `x` / blank, `❄` → `*`, `☾` → `○`, `†` → `%`.
- **The map reacts.** If the widget changes what the map shows, draw that change on
  the fake world, otherwise the user is judging a menu, not a feature.
- **Blurb per variant**: one paragraph above the terminal naming the idea, its keys
  (`<kbd>`), and what it trades off. Write it as if it were the PR description.
- **Status bar is honest**: it lists exactly the keys the variant handles, in the
  `[key] label` spelling from `keys.md`.
- **Keep it one file**, no external scripts, so it can be sent and reopened anywhere.

Typical iterations the user asks for, and what they mean here: "combine 2 and 3"
(new variant, keep the old ones), "move X to a tab / remove the number keys" (edit
the variant in place, update its blurb and status bar), "make two animated versions"
(new variants on the tween engine; animate the modal with `withClip` offsets, animate
the map with a per-layer alpha; keep the 20 fps default so motion is honest).

## 3. Verify in the browser pane

Read `references/verification.md` before driving it: the pane cannot drive
`file://` tabs (serve the scratchpad with `python3 -m http.server` in the background),
the pane has a tab cap and a named fallback when it is full,
Space and Return do not arrive from the key tool (dispatch `KeyboardEvent`s with
`javascript_tool`), screenshots lag one action (wait a second, or read state back),
and a held tween freezes an animation for a screenshot. Drive every key of every
variant, run `checkGlyphs()` in each state, and check the console for errors and
`not CP437` warnings before sending anything.

## 4. Send, then iterate

`SendUserFile` the HTML as soon as all variants work, with a one-line caption, and
again after each meaningful change. Say in the final message which tab shows what,
and name the one caveat the user should know (a browser quirk, a fidelity
compromise). Offer, in one line, to publish it as an artifact if they want a link;
do not publish unasked.

## 5. Hand off the chosen variant

When the user says "write the requirements" or "plan variant N", read
`references/handoff.md`. It covers pulling cell-exact renders with `dump()`, the
screen spec shape (`docs/screens/_template.md` plus geometry tables, precedence
rules, every key, effects on other screens, and the component sheet each part maps
onto), the plan shape (`docs/overlay-switcher-plan.md`: decisions, clippy-checked
data model, steps that each end green with cited line numbers, a render-parity
gate), CP437 substitutions the code will force (`›`→`»`, `…`→`·`), and how to
revalidate the plan against `main` before anyone builds from it.
