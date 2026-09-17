# Component Name

Back to the [component index](README.md).

One or two sentences: what the component is, and which screens use it (link the
screen files under `../screens/`).

## Anatomy
A template render with every slot written as `<Name>`. Canonical width unless
the component is inherently wider; state the width if it differs.

```
<template>
```

## Slots
| Slot | Required | Position | Overflow |
|------|----------|----------|----------|
One row per `<Name>` in the anatomy. *Position* says where the slot sits and how
it aligns. *Overflow* says what happens when there is not enough room: cut,
dropped, wrapped, or clipped, and in what order slots give way.

## Sizing
Width and height rules: fixed, fills the area, or computed from parameters.
Minimum size. Which widths are parameters and their defaults. Note that the
component never writes outside its area.

## Variants
| Variant | What changes | Used by |
|---------|--------------|---------|
Every distinct look the component can take. Colour-only variants count.

## Interaction
| Key | Standard | Here |
|-----|----------|------|
The keys this component responds to, one row each, against the standard in
[keys.md](keys.md). *Standard* is the meaning from that sheet; *Here* is what
the key does to this component. Keys are handled by the screen; the component
draws the resulting state. Write "None. Display only." for components that
take no input.

## Styling
Which `theme` item each slot uses. Name the style function (`theme::title()`)
or constant (`theme::DIM`), never an RGB value. Say what is background-filled.

## Glyphs
Every glyph the component draws, by `glyphs::` constant. Glyphs that come from
ratatui (box-drawing borders) are named as such. All must be CP437 and one cell
wide.

## Composition
*Contains:* components that may appear inside this one.
*Contained by:* components or screen layouts this one appears in.

## API
### Today
Exact path and signature of the helper that draws this now, or the screen
function that hand-rolls it.

### Planned
The first-class struct and its builder calls, following the API shape in the
[README](README.md#api-shape).

## Gaps today
Where the current code differs from this spec. "none" if it matches.

## Examples
Each example under its own heading naming the variant, with the width in
parentheses. Pixel-exact: every example must be reproducible by the component.
Row components are shown inside a panel border so spacing is visible.

### Simple (43 columns)
```
<render>
```

## Open questions
Anything this sheet does not settle. "none" if nothing is open.
