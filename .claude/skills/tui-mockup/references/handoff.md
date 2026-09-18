# From a chosen variant to a spec and a plan

Once the user picks a variant (usually after two or three rounds of "combine 2 and
3", "move that to a tab", "remove the number keys"), the mockup's job is to feed two
documents in the house style. The S14 Overlay Switcher went through exactly this path:
`docs/screens/s14-overlay-switcher.md` and `docs/overlay-switcher-plan.md` are the
worked examples, and the feature shipped from them.

## 1. Pull cell-exact renders out of the mockup

Set the state you want, render, and dump the region. Do this in `javascript_tool`
against the served page:

```js
Object.assign(VARIANTS[state.v].st, {open:true, cur:4}); render();
dump(10, 30, 14, 97)          // rows 10–30, columns 14–97
```

Paste the result into a fenced block in the spec. Cell-exact examples are what make
a screen spec checkable later, and the render tests (`ui::tests::regenerate_screen_renders*`)
will produce the same shape once built.

## 2. The requirements document: `docs/screens/sNN-<name>.md`

Follow `docs/screens/_template.md`: Purpose, Variants, Layout, Content requirements,
Glyphs and colors, Interaction, States and edge cases, Open questions. Add a Status
line at the top ("specified, not yet built", the mockup variant it came from). Things
the mockup settles that the template does not ask for, and that the builder needs:

- **Geometry as inner columns and rows** (a table of regions), plus the absolute
  position on the 155×45 frame for the default panel split and for the wide view.
- **A per-row anatomy sentence**: blank, mark at 2–4, name at 6 padded to 11, and so
  on. The builder lays the row out from this sentence.
- **Composition and precedence rules** when the feature stacks things (draw order,
  which colour wins on a creature, what fades when).
- **Every key**, inside and outside the modal, including the ones retired.
- **Effects on other screens**, itemised: status-bar hints, sidebar sections that go,
  the map title, help text, which S12 button now does what.
- **State**: what is remembered, where it lives, what survives `Esc`.
- **Components it maps onto**: name the sheet in `docs/components/` for each part
  (Modal, Checkbox, Table, Legend, Divider, Text) and say where the mockup differs
  from the sheet. The sheet wins; note the difference in the spec, do not carry the
  mockup's look into the code.

## 3. The plan: `docs/<feature>-plan.md`

Same shape as `docs/file-split-plan.md` and `docs/overlay-switcher-plan.md`:

- **Goal, non-goals, starting point** with current file sizes for every file the work
  touches (`wc -l`), because the 800-line ceiling is absolute and two files are
  usually within a step of it.
- **Design decisions** D1…Dn, each with its reason. Typical ones: where the state
  lives (a pushed modal only sees `&mut AppState`), one type replacing the old enum,
  an undimmed backdrop needing a hook on `Screen`, built from components.
- **Data model** as Rust, checked against clippy pedantic before writing it down:
  `Option<Option<T>>` trips `option_option` (use a small enum), more than three
  `bool` fields trips `struct_excessive_bools`, six arguments is the limit.
- **Steps that each end green**, pure code motion first (split the files near the
  ceiling), then the type, the renderer, the state move, the keys, the sidebar, the
  screen, the docs and renders, verification. Cite files and line numbers for every
  test and call site the step rewrites; a render-parity gate ("single-layer renders
  regenerate with no diff") proves a refactor changed nothing.
- **Test plan** table by tier, matching `AGENTS.md`: gate, unit by module, the
  components tier for anything under `src/widgets/` or `docs/components/`, chunk only
  if `src/sim` moved.
- **Risks** the plan already handles, and follow-ups it leaves out.

Glyphs: the mockup can use any Unicode; the code cannot. `›` and `…` are not CP437
and became `»` (`glyphs::CUE`) and `·` in S14. Check every non-ASCII glyph in the spec
against `tests::all_glyphs_are_cp437` before the plan promises it.

## 4. Revalidate before anyone builds

Plans go stale in a day when other branches merge. Before handing over, or when asked:

```bash
git fetch origin main
git log --oneline HEAD..origin/main
git diff --stat HEAD...origin/main -- src/ui src/widgets docs AGENTS.md
```

Then re-check every number the plan cites: `wc -l` on each file, `grep -n` for each
test and call site, the checksum and save `VERSION` in `AGENTS.md`, and whether a new
tier or convention appeared (the component system added `cargo test --test components`
and "new code uses the components"). Record the commit you validated against at the
top of the plan and list what changed.

## 5. What to send

`SendUserFile` the mockup HTML when it first works and after each meaningful change,
with a one-line caption. Do not commit the mockup; it lives in the scratchpad. The spec
and plan are ordinary docs commits and a PR against `main`, docs only, with the open
questions called out in the PR body.
