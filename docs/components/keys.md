# Keys

Back to the [component index](README.md).

The standard keypresses every interactive component follows. A screen owns key
handling (`Screen::handle_key`); a component only draws the state the screen
keeps. That split stays in the planned system: a component never reads keys.
This sheet is what a screen maps a key *to*, so every screen agrees.

Each component sheet has an **Interaction** section that lists the keys it
responds to against this table. Where the live code differs, the difference is
in [Deviations today](#deviations-today) below and under that sheet's Gaps.

## Standard bindings

| Key            | Meaning                                                                 | Components                                                        |
|----------------|-------------------------------------------------------------------------|-------------------------------------------------------------------|
| `Enter`        | select: activate the focused item, confirm a dialog, apply typed input  | [Menu](menu.md), [Button Row](button-row.md), [Table](table.md) row, [Stepper](stepper.md) and [Text Field](text-field.md) typing |
| `Space`        | select, the same as `Enter`; on a focused [Checkbox](checkbox.md) it toggles | Menu, Button Row, Checkbox                                    |
| `Esc`          | back or close: close a [Modal](modal.md), leave a data screen, cancel typing without applying | Modal, every data screen, Stepper and Text Field typing |
| `q`            | exit the application, after a confirm when the world has unsaved changes | global                                                           |
| `←` `→`        | step a value down or up; move focus along a [Button Row](button-row.md); move the cursor in a Text Field; scroll the map sideways | Stepper, Button Row, Text Field, map |
| `↑` `↓`        | move the selection or scroll one row                                    | Menu, Table, [Scroll Region](scroll-region.md), map               |
| `PgUp` `PgDn`  | scroll one page                                                         | Scroll Region, Table                                              |
| `Home` `End`   | jump to the first or last row                                           | Scroll Region, Table                                              |
| `Tab` `BackTab`| move focus to the next or previous field, button or panel               | Stepper and Text Field forms, multi-panel screens                 |
| `1`–`9`        | pick the n-th item of a strip, chart or speed list                      | [Filter Strip](filter-strip.md), S05 chart pick, S10 speed        |
| `=` `-`        | on the main screen (S01 and its overlays and modes): speed up or slow down the simulation. On the charts: zoom the time window in or out. Nowhere else. Shift is optional: `+` acts as `=` and `_` acts as `-` | S01 world map; [Chart](chart.md) |
| `.`            | step the simulation one tick                                            | main screen only                                                  |
| `?`            | help                                                                    | global                                                            |
| letter keys    | a screen-specific action or toggle, always shown in a [Key Hint](key-hint.md) | Checkbox, Filter Strip, [Status Bar](status-bar.md)         |

## Rules

1. **One meaning per key.** A key that selects in one place never closes in
   another. `Esc` never destroys and never confirms; it cancels or closes.
2. **`Enter` and `Space` are interchangeable for select.** The one addition is
   that `Space` on a focused Checkbox toggles it. On the world map nothing is
   focused, so `Space` there pauses and resumes the simulation; that is the only
   place `Space` means something other than select.
   `=` and `-` are the one pair whose target depends on the screen: they
   always mean more and less, of speed on the main screen and of zoom on the
   charts. Every other screen ignores them; speed is not a global control.
   Shift is never required: `+` and `_` are accepted as `=` and `-`.
3. **Arrows do not cross roles.** `←` `→` change a value or move along a row;
   `↑` `↓` move between rows. Neither changes panels; `Tab` does that.
4. **Every handled key is visible.** A screen shows each key it handles in its
   Status Bar or in a Key Hint next to the thing it acts on.
5. **Quit is confirmed, never silent.** `q` asks first when there are unsaved
   changes, using the confirm [Modal](modal.md).

## Key Hint spelling

The Status Bar and Key Hints write keys exactly like this, so the same key
looks the same on every screen.

| Key           | Written as | Key            | Written as |
|---------------|------------|----------------|------------|
| Enter         | `[Enter]`  | Space          | `[Space]`  |
| Escape        | `[Esc]`    | Tab            | `[Tab]`    |
| Left / Right  | `[←→]`     | Up / Down      | `[↑↓]`     |
| Page Up / Down| `[PgUp]` `[PgDn]` | Home / End | `[Home]` `[End]` |
| More / Less   | `[-/=]`    | a range        | `[1-5]`    |
| a set         | `[a/b/c]`  | a letter       | `[k]`      |

## Deviations today

Where the live code disagrees with the table above. Each row is also listed
under the named sheet's Gaps today.

| Where                    | Key       | Today                                              | Standard                              | Sheet |
|--------------------------|-----------|----------------------------------------------------|---------------------------------------|-------|
| any screen with a world  | `q`, `w`  | return to the title screen (confirm if dirty)      | `q` exits the application; `w` unassigned | [Menu](menu.md) |
| S10 Controls modal       | `Space`   | toggles pause                                      | select; nothing is focused, so pause is acceptable under rule 2 only if S10 gains no focus list | [Modal](modal.md) |
| S12 Alert modal          | `Space`   | dismisses the alert                                | activates the focused button          | [Button Row](button-row.md) |
| S09 World Generation     | `Space`   | opens typed entry on a numeric Stepper             | select; typing should start on the first digit key | [Stepper](stepper.md) |
| S09 World Generation     | `Enter`   | generates the world from any field                 | activates the focused field or button only | [Stepper](stepper.md) |
| S03 Inspector, S04 Species | `←` `→` | switch panel or pane                               | `Tab` switches panels                 | [Table](table.md) |
| S04 species detail       | `↑` `↓`   | change species                                     | `←` `→` step through species, `↑` `↓` scroll | [Table](table.md) |
| every screen with a world | `+` `-` `.` | the app-level fallback table changes speed and steps a tick from any screen that does not consume them, and S10 handles them itself | speed and step keys work on the main screen only | [Modal](modal.md) |
| S01 map, S05 Charts      | `=` `_`   | not handled; only `+` and `-` work                 | `=` and `_` accepted as `+` and `-`   | [Chart](chart.md) |
| Status Bar, S10 hint     | spelling  | `[+/-] speed`                                      | `[-/=] speed`                         | [Status Bar](status-bar.md) |
| S09 Text Field           | `←` `→`   | nothing; there is no cursor                        | move the cursor                       | [Text Field](text-field.md) |
| confirm dialog           | `n`       | answers no                                         | `Esc` is no; extra letters are allowed only when shown in a Key Hint | [Modal](modal.md) |
| S10 Checkboxes           | `↑` `↓`   | nothing; rows are letter-driven only               | `↑` `↓` move focus, `Space` toggles   | [Checkbox](checkbox.md) |

## Open questions

- `w` today also returns to the title. Keep it as a dedicated "to title" key
  once `q` means exit, or drop it?
- Should `Space` on the world map keep meaning pause, or move pause to `p` and
  free `Space` entirely? `p` opens the Controls modal today.
