# Verifying a mockup in the browser pane

The mockup is only worth sending once you have driven every variant yourself. These
are the quirks of the built-in browser pane that cost time the first time round.

## Serve it, do not open the file

A `file://` tab in the pane cannot be driven: screenshots, clicks and key presses all
fail with "This tab shows a local file". Serve the scratchpad instead and navigate to
`http://127.0.0.1:<port>/<file>.html`:

```bash
cd <scratchpad> && python3 -m http.server 8765 --bind 127.0.0.1
```

Run it in the background (`run_in_background: true`) and leave it up for the whole
session; the user keeps looking at the same tab after you hand the file over.
`preview_start` with a `url` is fine for opening; `navigate` on an existing tab is
fine for reloading after each edit.

## When the pane is full

The pane has a small tab cap and it is shared with any other agents in the session.
If `tabs_create` or `navigate` refuses with a tab-cap error, do not fight for a tab
and do not skip verification. Fall back, in this order:

1. `mcp__chrome-devtools__new_page` (load the tools with `ToolSearch` first) opens a
   separate page in the user's Chrome where the same `javascript_tool`-style checks
   run through `evaluate_script`; close the page afterwards.
2. Headless: run the mockup's own script in Node with a stub DOM
   (`document.getElementById` returning objects with `innerHTML`, `addEventListener`,
   `focus`, `dataset`, `classList`; `window.addEventListener` capturing the key
   handler; `requestAnimationFrame`/`setTimeout` as normal), then call
   `render()`, `checkGlyphs()`, `dump()` and the captured key handler directly. This
   proves keys, glyphs and geometry but not colours or the fit-to-width scale, so
   say so in the notes.

Whichever route you take, say in the final message which one it was.

## Keys that never arrive

`computer` `key` sends `Return` and `space` in a form the page sees as an empty key.
Arrows, letters, `Tab` and `Enter` (spelled exactly `Enter`) arrive normally. For
anything else, dispatch the event yourself with `javascript_tool`:

```js
const k=(key,shiftKey)=>window.dispatchEvent(new KeyboardEvent('keydown',{key,shiftKey:!!shiftKey}));
k('o'); k('ArrowDown'); k(' '); k('Enter');
```

The template's key handler normalises `e.code === 'Space'` and `'Enter'` too, so a
real browser works with the real keys; only the pane needs the synthetic route.

## Screenshots lag one action

A screenshot taken straight after a key or a JS call often shows the frame *before*
it. Add `{"name":"computer","input":{"action":"wait","duration":1}}` before the
screenshot in a `browser_batch`, or read the state with `javascript_tool` instead of
trusting the picture:

```js
JSON.stringify({v:state.v, ...VARIANTS[state.v].st})
```

## Freezing an animation

To photograph a mid-animation frame, hold the tween at a value instead of racing
the clock: `hold('modal',0.45); render();` then screenshot. Release with
`delete anims.modal; finals.modal=1; render();`. Held tweens never finish on their
own, so release every one before handing over or the page animates forever.

## The user may be in the pane too

The pane is shared. If the state you read back is not the state you set, the user
has probably been pressing keys in the same tab. Reload and re-drive rather than
"fixing" a bug that is not there.

## Glyphs: run the check, do not eyeball it

`●`, `—`, `…`, `›` and friends look fine in a browser and are rejected by
`tests::all_glyphs_are_cp437` the moment someone builds the thing. In every
variant and state:

```js
JSON.stringify(checkGlyphs())     // {} is the only acceptable answer
```

A non-empty result names the glyph, its code point and one cell where it appears;
replace the glyph in the variant's draw code with a CP437 one (`■ • ─ - · »` cover
almost every case) and re-run. The console also carries a `not CP437` warning the
first time a stray glyph is drawn, so read it after each edit.

## What to check per variant

- Every key the status bar advertises does what it says, and nothing else does
  anything (digits, letters that were retired).
- The map beneath a modal still renders correctly when the modal is open.
- No console errors (`read_console_messages` with `onlyErrors`).
- Long names and the widest sub-list still fit their columns; a value that is cut
  is cut, never overflowing into the next column.
- The blurb above the terminal matches what the variant actually does.
