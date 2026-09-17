# Spacer

Back to the [component index](README.md).

A blank block of a fixed number of rows or columns, for deliberate gaps inside
a [VStack](vstack.md) or [HStack](hstack.md). It paints nothing, so the
background stays whatever the enclosing [Panel](panel.md) filled.

## Anatomy

```
<blank>
```

## Slots
| Slot  | Required | Position  | Overflow                     |
|-------|----------|-----------|------------------------------|
| none  |          |           | clipped by the stack like any child |

## Sizing
`Spacer::rows(n)` is `n` rows tall and 0 wide; `Spacer::cols(n)` is `n`
columns wide and 0 tall. Inside a stack the other dimension is the stack's.

## Variants
| Variant | What changes            | Used by                                  |
|---------|-------------------------|------------------------------------------|
| Rows    | vertical gap            | blank rows between sidebar sections      |
| Cols    | horizontal gap          | the three cells between a bar and its value |

## Interaction
None. Display only.

## Styling
None. Paints nothing.

## Glyphs
None.

## Composition
*Contains:* nothing.
*Contained by:* [VStack](vstack.md), [HStack](hstack.md).

## API
### Today
No helper; screens add `row += 1` or leave cells unwritten.

### Planned
```rust
Spacer::rows(1)
Spacer::cols(3)
```
`height` is `n` for Rows and 0 for Cols; `min_width` is `n` for Cols and 0
for Rows.

## Gaps today
none

## Examples

### One blank row between sections (43 columns)
```
║ 14:00  ☼ day      ►► x2  running        ║
║                                         ║
║─ Resources ─────────────────────────────║
```

## Open questions
none
