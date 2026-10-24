# 256.app — iconography & art direction

**Canonical reference** for UI icons (function/system) and how they relate to **council avatar** art. Complements [`PRODUCT_GUIDE.md`](PRODUCT_GUIDE.md) § Brand identity and [`static/css/brand.css`](../static/css/brand.css) tokens.

**Last reviewed:** 2026-07-24 (Epic style plate for member onboarding avatars)

---

## Council avatars as the north star

Generated council portraits (`static/generated/council/<handle>/avatar-default.png`, 512×512 source, shown at **large** size in UI) establish the visual language the product should echo in every smaller glyph.

**Member onboarding avatars (2026-08-11):** `@epic` paints human avatars via Imagen subject customization + a **style plate** (`agents/council/rag/epic/style-plate-bust-v6.png`) for illustration medium — flat editorial bust on `#000000`, not photorealism. Style plate supplies **technique and composition only**; identity comes from the member's capture + colour card. See [`PRODUCT_GUIDE.md`](PRODUCT_GUIDE.md) § Onboarding portrait pipeline.

| Trait (from portrait pipeline) | UI implication |
|--------------------------------|----------------|
| **Humanist editorial illustration** — clear silhouettes, restrained texture | Function icons stay **readable at 16px**; avoid fine interior detail |
| **Warm, calm presence** — engaged, not anxious | Rounded caps/joins; open shapes; no aggressive sharp corners |
| **Limited palette** — ≤64 colours per portrait (`color_direction` in agent JSON) | UI icons use **stroke + `currentColor`**; reserve **full spectrum** for rings, globe, and brand lockups |
| **Symbolic centre motif** per councillor (anchor, scales, tree, etc.) | Each major action family may eventually carry a **single memorable motif** (not twelve variants) |
| **Spectrum frame** — conic gradient ring on avatars and toolbar | **Brand/system** icons may use spectrum **on the frame or one accent stroke**; **action** icons stay monochrome until hover |

Portrait prompts: `agents/council/<handle>.json` → `portrait_synthesis` (`style_direction`, `perspective_direction`, `color_direction`, optional `style_reference_image`). Shared defaults: `agents/council/shared/default_portrait_generation_prompt_template.txt`. Epic RAG: `agents/council/rag/epic/portrait_style_guide.txt`.

---

## Icon classes

| Class | Role | Colour model | Hover |
|-------|------|--------------|-------|
| **Identity** | Humans, brands, council (photos/illustration) | Full colour art inside spectrum frame | Frame highlight (blue/cyan mix); not yellow |
| **Brand lockup** | Smiley, robot loader, spectrum globe | Spectrum stroke/fill per asset | Per component (loaders animate) |
| **Function / system** | Post actions, rail create/search, edit, judicial affordances | `currentColor` = `var(--text)` default | **`var(--ui-system-icon-hover)` = sun-burst yellow** |
| **Semantic state** | Selected vote, feel heart, verdict accents | Palette semantics (yellow up-vote, red down, cyan thread count) | Unchanged — state colour, not hover |

**Rule:** Yellow hover is for **interactive function/system** icons only. Do not move link text or CTA buttons to yellow hover (links stay `--ui-action-hover` blue; `.btn-cta` keeps yellow **text** always).

---

## Size ladder (standard)

All new and redesigned icons MUST be authored on a **24×24** (or **40×40** for spectrum-heavy brand marks) **viewBox**, then scaled with CSS — never rasterize action icons.

| Token | px | rem (16px root) | Typical use |
|-------|-----|-----------------|-------------|
| `--icon-size-lg` | **48** | 3rem | Profile/community avatars (`--avatar-display-size`), godmother grid, public/private post headers, toolbar logo |
| `--icon-size-md` | **32** | 2rem | Post action bar, rail create/search glyphs, profile edit/redraw, touch-primary controls |
| `--icon-size-sm` | **16** | 1rem | Inline badges (private globe), thread/reply affordances, dense metadata |

**Touch targets:** Icon may be 16–32px but clickable control must still meet `--tap-min` (2.75rem) padding on mobile.

---

## Stroke & geometry (function icons)

Align the **post-actions** sprite and future system SVGs to council clarity:

| Property | Value |
|----------|--------|
| View box | `0 0 24 24` (action set) or `0 0 40 40` (spectrum brand marks) |
| Stroke width | **1.35–1.45** at 24px canvas (scale proportionally) |
| Caps / joins | `round` |
| Fill | Prefer **stroke-only**; filled hearts only for semantic “feel” |
| Optical padding | ~1px inset from viewBox edge so strokes are not clipped at sm size |

**Redesign checklist** (each symbol in [`static/icons/post-actions.svg`](../static/icons/post-actions.svg)):

1. Vote up/down — keep speech-bubble + thumb marriage; simplify bubble tail for 16px legibility  
2. Share / escalate — arrow + baseline; match stroke weight across pair  
3. Copy link — two rounded rects; equal corner radius  
4. Open case — clipboard; fewer interior lines at sm  
5. Reply / thread — single-path where possible  
6. Edit / avatar-redraw — pencil and bust+refresh; same visual weight  
7. Feel heart — filled; colour from feel key (not hover yellow)  
8. Private message — speech bubble + lines; pair with copy-link on profile headers  
9. Logout — fire-exit door frame + outward arrow; semantic **arterial red** on profile header (not yellow hover)

**Sprite cache:** browsers cache the SVG document referenced by `<use href="/static/icons/post-actions.svg#…">`. When adding or changing symbols, bump the `?v=` query on `SPRITE_HREF` in `src/ui/post_icons.rs` (and `rail.rs`, `nom_icon.rs`, `home-shell.js` for feed chrome nav) in the same change.

---

## Feed chrome nav set (`<<` · `<` · `>` · `^`)

Secondary nav paging and scroll-to-top share one **chrome nav** family — not post actions, not rail search/create:

| Control | Symbol | Placement |
|---------|--------|-----------|
| First page | `icon-feed-first` (`<<`) | Subnav pinned start; centered under logo when page &gt; 1 |
| Newer | `icon-feed-prev` (`<`) | Subnav pinned start |
| Older | `icon-feed-next` (`>`) | Subnav pinned end; centered under create-post |
| Back to top | `icon-feed-top` (`^`) | Fixed above footer; same column as older |

**Visual:** yellow `currentColor` chevron on `--bg` disc with subtle yellow ring (`.app-rail-slot-frame--feed-nav-action`). Hover/focus → `--ui-system-icon-hover` on glyph and ring. Buttons use `.app-rail-feed-nav-btn` + `.app-chrome-nav-btn` + `.system-icon-btn`.

**Alignment:** topbar create-post lives in `.app-topbar-action-cell` (mirrors `.app-topbar-logo-cell` width) so `>` aligns under `+`. Subnav end column (`.app-rail-pinned-end`) uses the same width token.

---

## Asset inventory

| Asset | Path | Size tier | Status |
|-------|------|-----------|--------|
| Post actions sprite | `static/icons/post-actions.svg` | md / sm | **Shipped** — Phase 2 stroke pass (1.4px, round caps); includes `icon-private-message` (2026-07-22), `icon-logout` (2026-08-14) |
| Noms wallet token | `post-actions.svg#icon-nom` via `src/ui/nom_icon.rs` | sm | **Shipped** — coin + N mark; tooltip "Noms" |
| Private audience globe | `static/icons/private-post-globe.svg` | sm | **Brand** — spectrum ring; keep; display at `--icon-size-sm` |
| Join In marketing rows | `static/icons/marketing-features.svg` | md (32px) | **Shipped** — shield, human+AI, ecology network, community; 2-col icon + copy grid |
| Rail search / create | `post-actions.svg#icon-rail-*` via `src/ui/rail.rs` | md glyph in lg frame | **Shipped** — `currentColor` + yellow hover on button |
| Feed chrome nav | `post-actions.svg#icon-feed-*` via `src/ui/rail.rs`, `static/js/home-shell.js` | md glyph in lg frame | **Shipped** — `<<` first, `<` newer, `>` older, `^` scroll-to-top; yellow chevron on dark disc (`.app-chrome-nav-btn`) |
| Toolbar smiley / robot | `src/ui/snippets.rs` | legacy 40px | **Brand** — spectrum; periodic sync with loaders |
| Council avatars | `static/generated/council/*/avatar-default.png` | lg | **Reference** — source of truth |
| Card loaders | `src/ui/snippets.rs` | md–lg | **Brand** — animated spectrum |

---

## Implementation map

| Concern | Location |
|---------|----------|
| Size tokens | `static/css/brand.css` `:root` — `--icon-size-lg`, `--icon-size-md`, `--icon-size-sm` |
| System hover | `--ui-system-icon-hover`; `.post-action-btn`, `.system-icon-btn` |
| Rust sprite helpers | `src/ui/post_icons.rs`, `src/ui/nom_icon.rs` |
| Agent mirror | `.cursor/rules/ui-iconography.mdc` |

---

## Redesign phases

1. **Shipped:** Tokens, hover yellow on function icons, md/sm sizing, documentation.  
2. **Shipped:** `post-actions.svg` symbols redrawn to stroke spec; `icon-rail-search` / `icon-rail-create` in same sprite; rail uses `<use>` + `currentColor`.  
3. **Shipped:** Brand smiley + robot + loader geometry in `static/icons/brand-*-markup.svg`; toolbar lockup at `--icon-size-lg` (48px); mouth brightens on logo link hover.

When council portrait style shifts (new `style_direction`), update this doc and re-run the Phase 2 sprite pass.

---

## Related

- [`PRODUCT_GUIDE.md`](PRODUCT_GUIDE.md) — brand palette & card accents  
- [`static/icons/README.md`](../static/icons/README.md) — file-level notes for designers  
- [`agents/council/AVC_PATTERN.md`](../agents/council/AVC_PATTERN.md) — persona, not pixels  
- [`static/generated/council/README.md`](../static/generated/council/README.md) — avatar file layout
