# Static icons

SVG sources for **function/system** UI (not council portraits — those live under `static/generated/council/`).

## Art direction

See [`docs/ICONOGRAPHY.md`](../../docs/ICONOGRAPHY.md). Council avatars define silhouette clarity, stroke weight, and palette discipline; this folder holds monochrome action glyphs that inherit `currentColor` and **yellow hover** via CSS.

## Files

| File | Symbols | Display tier |
|------|---------|----------------|
| `post-actions.svg` | vote, share, copy, case, escalate, reply, thread, feel, edit, avatar-redraw, rail-search, rail-create | md (32px) / sm (16px) |
| `marketing-features.svg` | safe, human-ai, ecology, community (Join In card) | md (32px) + statement grid |
| `brand-smiley-markup.svg` | site logo (human) — included by `src/ui/snippets.rs` | lg (48px) |
| `brand-robot-markup.svg` | site logo (AI / face capture) | lg (48px) |
| `brand-loader-markup.svg` | spinning loader face | md |
| `private-post-globe.svg` | audience / share spectrum globe | sm (16px) |

## Adding an icon

1. Draw in `viewBox="0 0 24 24"` with stroke width ~1.35, round caps.  
2. Add `<symbol id="icon-…">` to `post-actions.svg`.  
3. Expose via `src/ui/post_icons.rs`.  
4. Size with `.post-action-icon` / `.system-icon--md` — do not hard-code pixel widths in Rust unless necessary for layout.
