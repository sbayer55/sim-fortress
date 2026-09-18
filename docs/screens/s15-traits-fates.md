# S15 — Traits & Fates

Back to the [screen overview](README.md).

Status: built (S15a), from variant 1 ("the matrix") of the Traits & Fates mockup.
Render: [renders/S15a.txt](renders/S15a.txt).

## Purpose
S15 shows the link between genome and fate for one species. For each of the twelve traits it
shows how the trait relates to what happened to the creatures that carried it over the last
240 days: how long they lived, how many young they raised, how often they escaped or killed,
and what killed them. It also follows one trait across age among the living, so selection
shows up as the mean trait drifting with age. Crowded days can be compared with sparse ones.

S04 shows what a species' genome *is*. S15 shows what each trait *does*.

## Variants
| Id   | Variant                 | When it is shown |
|------|-------------------------|------------------|
| S15a | trait × outcome matrix  | On entry with `t` from the map or any screen that lets the global table through. Opens on the first species, all days. |

## Layout
A full-screen data screen. The body is rows 1–43 and the status bar is row 44. The body uses the map family's 112/43 split.

| Panel | Position | Size | Border |
|-------|----------|------|--------|
| Traits and outcomes: `<Plural>` | cols 0–111, rows 1–43 | 112 × 43 | Focus. Right-hand info `<n> lives in 240 days, <m> alive now` |
| Selected | cols 112–154, rows 1–43 | 43 × 43 | Outer |
| Status bar | row 44 | 155 × 1 | — |

Main panel, inner rows (inner row 0 is screen row 2), inner columns from the left border:

| Inner row | Content |
|-----------|---------|
| 0 | Species strip (cols 0–65) and crowding strip (cols 66–109) |
| 1 | Group labels, as dividers: `what they achieved` over columns 31–53, and `how they died: share of deaths` over columns 55–101 |
| 2 | Header row on `HEADER_BG`: `trait` at 1, `living` at 13, `mean` at 25, then the nine outcome headers centred in their cells. The selected outcome's header is in `KEY`. |
| 3–26 | Twelve trait rows, two rows each, starting at `3 + 2·t` |
| 28 | Divider `<Trait> across ages, living <plural>` |
| 29 | `age` and a day label on every fourth band (18 bands of 5 columns from column 13) |
| 30–31 | `living now`: a histogram of living count per age band |
| 32 | `mean <Abr>`: the mean trait value per band, on a `heat` ground |
| 34 | `died of`: the commonest cause's letter and its % of the band's deaths, `p100` included |
| 35 | The age-trend sentence |
| 36–40 | Divider `Key` and four key rows (the last explains why OldAge is uncoloured) |

**Trait row anatomy.**
- Cursor `»` at 0 on the selected row, which is also filled with `SELECT_BG` across columns 0–29.
- Trait name at 1, padded to 11.
- A 10-bucket, 2-row histogram of the living over the trait's range among the lives (min and max widened to .05, at least .10 wide), at 13–22, in the species colour.
- Mean of the living (`.55`) at 25.
- Nine cells at `31 + 8·o`, each 7 × 2 with a one-column gap.
  - The top row holds `r` (`+.42`, `-.05`, `+1.0`), `n/a` (fewer than 30 lives) or `none` (every life had the same outcome, for example no disease deaths), centred.
  - The second row holds `↑` or `↓` at the cell's column 3 when the effect is visible.
  - The ground is `theme::diverge_bg(±intensity)`: green where the trait helps, red where it hurts. Intensity is (|r| − .05) / .40, clamped to 0..1. OldAge cells keep the panel ground (see Content requirement 2).
  - Text is `TEXT_BRIGHT` and bold above .6 intensity, `TEXT` above .15, `DIM` below.
  - The selected cell is bracketed by `▐` and `▌` in `BORDER_FOCUS` in the gap columns on both rows.

**Sidebar (inner width 41, text at 1).** Sections separated by blank rows:
1. `Cell`: `<Trait> against <outcome>`; then `too few lives to say (under 30)` when n < 30 (this check comes first); else `none: <what the flat column means>` (`no counted death was starvation`, `all lives had 0.0 young per year`); else `r ±.nn  <strength>, the trait helps|hurts`, or `…, a neutral outcome` for OldAge; then `from <n> death(s)|life/lives`.
2. `By trait third`: three `LabeledBar`s (`low  <.49`, `mid  .49-.61`, `high >.61`) with the mean outcome per third, scaled to the largest, in the species colour darkened, as is and lightened.
3. `In words`: one sentence comparing the top and bottom thirds, wrapped to at most four rows. When the cell has fewer than 30 lives it says so, and when the outcome does not vary it says that instead.
4. `Crowding`: the same cell's `r` on all days, crowded days and sparse days, drawn as bars either side of a `│` at column 21 (9 cells at |r| = .6), then `less ◄   ► more`.
5. `Strongest links, <plural>`: the six shown cells with the largest |r|: trait, `↑`/`↓` plus the outcome in `GOOD`/`BAD`, and `r`. The selected cell's row has `SELECT_BG`.
6. `Population`: `alive now <n>  died <m>`, then a 39-cell bar of deaths by cause.

## Content requirements
1. **Lives.** The analysis runs over one record per creature of the species that lived in the window: the living set (age now), plus the deaths of the last 240 days from `Lineage::lives()`, the life log (`sim::lineage::lifelog`). The log is written by `behavior::kill`, which snapshots the genome, birth day, death day, cause, offspring, kills, attempts, chased and escaped. It is saved with the world (save format 15), keeps the last 240 days, and is capped at 50 000 records.
2. **Outcomes** (`sim::stats::outcomes::OUTCOMES`):

   | Column | Value | Who counts |
   |---|---|---|
   | Life | age at death | the dead |
   | Young | offspring × 365 / adult days | lives with at least 45 adult days; adult age from `adult_age_days` |
   | Escape (prey) / Kills (predators) | escaped / chased, or kills / attempts | lives with a denominator of at least 2 |
   | Starve, Thirst, OldAge, Preyed, Disease, Injury | 1 if that was the cause of death, else 0 | the dead |

   Life, Young and Escape/Kills are good and the other causes are bad (`Outcome::tone`). OldAge is
   **neutral**: within a 240-day window a long-lived creature is *less* likely to have died of old age,
   so scoring it either way would call a trait that works one that hurts. Its cells are uncoloured and
   its links are drawn in `TEXT`.
3. **Cells.** A cell holds the Pearson r of the trait against the outcome over the lives that count (`stats::pearson`, lag 0). It is shown from 30 lives; below that it is `n/a`, and when the outcome does not vary it is `none`.
4. **Crowding.** Take the median of the species' population over the last 240 daily samples. A life is *crowded* when the population on its death day (today, for the living) was above that median. The *crowded* and *sparse* filters keep only lives whose day is in the series; *all days* keeps everything.
5. **Age strip.** There are 18 bands, whose width is the 98.5th-percentile age ÷ 18, and the last band holds everything older. Each band shows the living count, the mean trait of the living when there are at least three, and the commonest death with its share. The sentence compares the mean trait of the youngest and oldest fifth of the living (at least 20 living). It only claims a direction when the gap is more than twice its standard error, sd × √(2 / n), where sd is the trait's standard deviation among the living and n the fifth's size; otherwise it reads `within noise, no sign of selection with age.`
6. **Caching.** The lives and the three matrices (one per crowding filter) are rebuilt once per sim day, or when the species changes. Moving the cursor or changing the filter redraws from the cache.

## Glyphs and colors
- `»` (`glyphs::CUE`) marks the row; `▐` `▌` (`glyphs::HALF_RIGHT`, `HALF_LEFT`) bracket the cell.
- `↑` `↓` show the direction of the effect; `◄` `►` appear in the key; `█` `▄` are the histograms; `·` marks an empty band; `│` is the crowding axis.
- Cause letters, and their colours in the age strip, key and population bar:

  | Letter | Cause | Colour |
  |---|---|---|
  | `s` | starved | `WARN` |
  | `t` | thirst | `INFO` |
  | `a` | old age | `ROCK_FG` |
  | `p` | preyed on | `BAD` |
  | `d` | disease | `SICK` |
  | `i` | injury | `MAGENTA` |

- Grounds: `theme::diverge_bg` for the cells, `dim(heat(t), .55)` for the per-band means, and `HEADER_BG`/`HEADER_FG` for the header row.

## Components
| Part | Component | Differences from the mockup |
|---|---|---|
| Panels | [Panel](../components/panel.md) (Focus, Outer) | — |
| Section rules, group labels | [Divider](../components/divider.md) | Group labels are left-aligned dividers, not centred |
| Species strip | [Filter Strip](../components/filter-strip.md) | Chips show the roster glyph (`V vole`), not the digit; the hint reads `[1-6] species` |
| Crowding chips | [Filter Strip](../components/filter-strip.md) | Blank glyph; hint `[c] days` |
| Living histograms, living-by-age | [Histogram](../components/histogram.md) | `rows(2)`, `col_w(1)` or `col_w(4)` |
| Third bars | [Labeled Bar](../components/labeled-bar.md) | Bracketed bar, four-cell value; the label is `TEXT`, not the third's colour |
| Lines | [Text](../components/text.md) | — |
| Status row | [Status Bar](../components/status-bar.md) | — |
| Matrix cells, crowding bars, population bar | drawn into the buffer | No component yet |
| Wrapped sentence | a private `wrap()` in `s15_traits/sidebar.rs` | Gap: there is no wrapping Text |

## Interaction
| Key | Action | Goes to |
|-----|--------|---------|
| `↑` `↓` | previous / next trait (wraps) | — |
| `←` `→` | previous / next outcome (wraps) | — |
| `1`–`9` | pick the species by roster position; digits past the roster do nothing | — |
| `c` | cycle all days → crowded → sparse | — |
| `Esc` | back to the screen S15 was opened from (the map, or a data screen such as S05) | [S01](s01-world-map.md) |

Every other key falls through to the global table (`Space`, `+`/`-`, `?`, other screens). The status bar reads `[↑↓] trait  [←→] outcome  [1-N] species  [c] crowding  [Esc] back`, with `<Species> · <filter>` on the right.

**Effects on other screens:**
- `t` is added to the global table next to `g` and `s`.
- The S01 sidebar's Notable section gains a row, ` [s] species · [t] traits & fates`, because the S01 status bar is full.
- The S11 Screens group gains `t  traits & fates`.

## States and edge cases
- **New world, no deaths yet.** Every cause column reads `n/a` or `none`, the population bar reads `no deaths in the window`, and the lifespan cells are `n/a`.
- **Extinct species.** Still selectable. It has no living members, so the histograms and age strip are empty and the sentence reads `Too few living to compare the young with the old.`; its deaths from the last 240 days are still analysed.
- **Small species** (wolves, lynx). Many cells are `n/a`. The sidebar says so rather than showing a number.
- **A cause nobody died of.** The column reads `none`. It is not a "no link" zero.
- **Rosters of more than nine species.** Only the first nine can be picked (see Open questions).
- **After a load.** The log is part of the save, so the matrix is the same as before saving.

## Open questions
- Rosters of more than nine species need a way to reach the tenth and later species (a
  cycle key, or reuse of S04's species picker).
- Opening on the followed creature's species, or on the most numerous one, may be friendlier
  than opening on the first species. Today the default world's voles are often extinct by year 1.
- A wrapping Text component would replace the sidebar's private `wrap()`.
