# notes-server — Frontend Design Spec

> Art direction: **"Phosphor & Paper"**
> Stack: React 19 · TypeScript · Vite · `motion/react` · Tailwind CSS v4 · lucide-react
> Status: implementation-ready. Every number here is a decision; change it on purpose, not by accident.

---

## 0. TL;DR for the engineer

- The whole site grows out of one real object: the **8×13 blue LED matrix** on the Arduino UNO Q. Its dot pitch sets the background grid. Its 8 brightness levels set the glow scale. Its 3×5 font draws course monograms and the 404 page. Its live bar graph sits in the header, the hero and the footer.
- Two themes, one board. **Dark = "Lab at night"** (blue-black ink, glowing LEDs). **Light = "Graph paper"** (warm paper, ink-blue dot grid). The LED plate stays a dark physical board in both themes.
- Motion runs on **springs**, uses **shared elements** (card → page → lightbox), and uses **quantized LED fades** (`steps(7)`) as a recurring digital tic. Everything animates transform/opacity only, except a small number of opacity-driven "glow" layers.
- The upload sheet morphs out of the CTA. A successful upload plays the firmware's arrow animation, which bursts into LED particles, and then the sheet morphs into the new note.

---

## 1. Concept & art direction

### 1.1 The idea

A small blue LED panel glows in a dark classroom, and its light falls onto a stack of handwritten notes. That is the whole brand: **phosphor** (precise, digital, alive, quantized) meeting **paper** (warm, human, messy, photographed). The UI chrome is the phosphor. The students' photos are the paper. The chrome stays crisp and restrained so that the notes, which are the actual content, look warm and tactile inside it.

### 1.2 Mood words

Nocturnal · precise · warm-under-glass · playful-but-quiet · "a well-made instrument".

### 1.3 References (what to steal, specifically)

| Reference | Take this |
|---|---|
| Linear (2023+ site) | Low-chroma dark surfaces, 1px inner highlights, restraint with color |
| Teenage Engineering (OP-1, product pages) | Hardware-as-UI, silkscreen labels in mono caps, physical plate objects |
| Vercel / Geist | Typographic rigor, mono for data, tight grids |
| Apple Photos (iOS) | Lightbox physics: swipe-down-to-dismiss with scale + backdrop fade, pinch zoom |
| Rauno Freiberg's interaction studies | Morphing surfaces (button → sheet), cursor-reactive spotlight borders |
| Dieter Rams / Braun dot displays | Dot-matrix as a layout primitive, not only decoration |

### 1.4 Why it fits

- It is **true**: the site really is served by a board with that exact LED panel. The visual identity matches the physical object on the desk, so it never feels like a stock theme.
- Dots are **cheap to render** (CSS radial-gradient, canvas sprites), so the identity costs almost nothing in performance.
- Blue + warm paper avoids the purple-gradient SaaS look and keeps student photos (white paper, blue/black ink, whiteboards) looking natural. The chrome color is the same family as ballpoint ink.

### 1.5 Signature elements (these five make it recognisable)

1. **LEDMatrix plate**: a faithful virtual copy of the board that mirrors `/api/stats` live, using the same algorithm as the firmware (§5.3).
2. **LED dot field**: a canvas hero background of quantized glowing dots that ripple on every real request (§4.9).
3. **LED monograms**: each course gets a 13×8 monogram of its initials drawn with the firmware's 3×5 font (§6.6).
4. **LED halftone covers**: course and chapter cover photos show through a dot mask and reveal the true photo on hover (§6.2).
5. **Quantized glow**: glows fade in 7 discrete steps (`steps(7)`), the way the hardware's 8 brightness levels do. Use this only for LED things; everything else uses springs.

---

## 2. Design tokens

All raw tokens are CSS custom properties on `:root` (light is the default) and `[data-theme="dark"]`. Map them into Tailwind v4 with `@theme inline` so utilities resolve at runtime.

```css
@custom-variant dark (&:where([data-theme="dark"], [data-theme="dark"] *));

@theme inline {
  --color-bg-0: var(--bg-0);            /* → bg-bg-0 */
  --color-bg-1: var(--bg-1);
  --color-surface-1: var(--surface-1);
  --color-surface-2: var(--surface-2);
  --color-surface-3: var(--surface-3);
  --color-line-subtle: var(--border-subtle);
  --color-line: var(--border);
  --color-line-strong: var(--border-strong);
  --color-fg-1: var(--text-1);
  --color-fg-2: var(--text-2);
  --color-fg-3: var(--text-3);
  --color-fg-4: var(--text-4);
  --color-accent: var(--accent);
  --color-accent-fg: var(--accent-fg);
  --color-accent-soft: var(--accent-soft);
  --color-hi: var(--highlighter);
  --color-ok: var(--success);
  --color-warn: var(--warning);
  --color-err: var(--danger);
  --font-display: "Bricolage Grotesque Variable", ui-sans-serif, system-ui, sans-serif;
  --font-sans: "Geist Variable", ui-sans-serif, system-ui, sans-serif;
  --font-mono: "Geist Mono Variable", ui-monospace, "SFMono-Regular", monospace;
  /* radii, shadows, spacing, breakpoints: see below, same names */
}
```

Theme resolution: before first paint, an inline script in `index.html` reads `localStorage["theme"]` (`"light" | "dark" | "system"`, default `"system"`), resolves `system` via `matchMedia("(prefers-color-scheme: dark)")`, and sets `data-theme` on `<html>`. That prevents a flash of the wrong theme. Also set `<meta name="color-scheme" content="light dark">` and update `<meta name="theme-color">` per theme (`#05070D` / `#F6F4EE`).

### 2.1 Color: Dark theme, "Lab at night"

| Token | Hex | Use | Contrast notes |
|---|---|---|---|
| `--bg-0` | `#05070D` | Page background | — |
| `--bg-1` | `#0A0F1A` | Section bands, footer | — |
| `--surface-1` | `#0F1624` | Cards | — |
| `--surface-2` | `#151E30` | Raised cards, inputs, sheet | — |
| `--surface-3` | `#1C2740` | Hover fills, pressed, tooltips | — |
| `--border-subtle` | `#1A2438` | Dividers, card rest border | — |
| `--border` | `#26334D` | Inputs, hover border | 3.1:1 vs bg-0 (non-text UI OK) |
| `--border-strong` | `#3A4B6E` | Focused containers, drag targets | — |
| `--text-1` | `#EAF0FA` | Primary text | 17.6:1 on bg-0, 14.9:1 on surface-2 |
| `--text-2` | `#A9B6CC` | Secondary text | 9.6:1 on bg-0, 8.1:1 on surface-2 |
| `--text-3` | `#8090AD` | Meta, captions | 6.1:1 on bg-0, 5.2:1 on surface-2 (AA) |
| `--text-4` | `#4A5874` | Disabled only (exempt) | — |
| `--accent` | `#4C9AFF` | LED blue: links, primary button fill, focus | 7.1:1 on bg-0 |
| `--accent-fg` | `#031026` | Text on accent fill | 6.6:1 on `#4C9AFF` |
| `--accent-hover` | `#6AADFF` | Primary hover | — |
| `--accent-soft` | `rgba(76,154,255,0.14)` | Selected rows, soft badges | — |
| `--accent-deep` | `#1E5BFF` | Glow color (never text) | — |
| `--focus` | `#7DB5FF` | Focus ring | 9.0:1 on bg-0 |
| `--highlighter` | `#FFD84A` | "New" badges, success accents, `::selection` bg | 14:1 on bg-0; text on it `#1A1400` |
| `--success` | `#3DD68C` | Success toasts/icons | 10.5:1 on bg-0 |
| `--warning` | `#FFB547` | Warning (disk nearly full, 507) | 11.2:1 on bg-0 |
| `--danger` | `#FF6470` | Errors | 6.9:1 on bg-0 |
| `--danger-soft` | `rgba(255,100,112,0.12)` | Error field fill | — |
| `--grid-dot` | `rgba(122,150,200,0.10)` | Page dot grid | decorative |
| `--scrim` | `rgba(3,5,10,0.72)` | Modal backdrop | — |

### 2.2 Color: Light theme, "Graph paper"

| Token | Hex | Use | Contrast notes |
|---|---|---|---|
| `--bg-0` | `#F6F4EE` | Warm paper page | — |
| `--bg-1` | `#EFECE4` | Bands, footer | — |
| `--surface-1` | `#FFFFFF` | Cards | — |
| `--surface-2` | `#FBFAF6` | Inputs, sheet | — |
| `--surface-3` | `#F1EEE6` | Hover, pressed | — |
| `--border-subtle` | `#E7E2D7` | Dividers, card rest | — |
| `--border` | `#D5CFC1` | Inputs | — |
| `--border-strong` | `#A9A293` | Focused containers | 3.0:1 on surface-1 |
| `--text-1` | `#121826` | Primary | 16.2:1 on bg-0 |
| `--text-2` | `#3F4859` | Secondary | 8.8:1 on bg-0 |
| `--text-3` | `#5E6778` | Meta | 5.1:1 on bg-0, 5.7:1 on white |
| `--text-4` | `#A3A9B4` | Disabled only | — |
| `--accent` | `#1F5EFF` | Ink blue: links, primary fill, focus | 5.1:1 on white, 4.6:1 on bg-0 |
| `--accent-fg` | `#FFFFFF` | Text on accent | 5.1:1 |
| `--accent-hover` | `#1748CC` | Primary hover | — |
| `--accent-soft` | `rgba(31,94,255,0.09)` | Selection fills | — |
| `--accent-deep` | `#1F5EFF` | Glow color (low alpha only) | — |
| `--focus` | `#1F5EFF` | Focus ring | — |
| `--highlighter` | `#FFE16B` | Highlighter marker bg ("New", `::selection`) | text-1 on it 14.8:1 |
| `--success` | `#0E8A56` | | 4.7:1 on bg-0 |
| `--warning` | `#9A5B00` | | 5.3:1 on bg-0 |
| `--danger` | `#C8283A` | | 5.4:1 on bg-0 |
| `--danger-soft` | `rgba(200,40,58,0.08)` | | — |
| `--grid-dot` | `rgba(31,60,120,0.12)` | Graph-paper dots | decorative |
| `--scrim` | `rgba(18,24,38,0.55)` | Modal backdrop | — |

### 2.3 LED colors (shared by both themes: the plate is always a dark board)

Firmware brightness levels 0–7 map to these perceptual colors. Hardware LEDs are very non-linear, so level 2 (the level the firmware uses for bars) must still read clearly on screen.

| Level | `--led-N` | Glow sprite | Firmware usage |
|---|---|---|---|
| 0 (off) | `#0D1628` | none | unlit dot (always drawn; the grid must be visible) |
| 1 | `#16305F` | none | — |
| 2 | `#2457B4` | none | bar graph body, waiting dot, stopped dash |
| 3 | `#2E6CD8` | none | comet tail |
| 4 | `#3A81F1` | 0.15α, r×1.8 | comet tail |
| 5 | `#4C9AFF` | 0.25α, r×2.2 | comet tail |
| 6 | `#8CBEFF` | 0.35α, r×2.6 | comet tail |
| 7 (max) | `#DCEBFF` core | 0.55α `#4C9AFF`, r×3.2 | heartbeat, upload tops, arrow, X, text |

Plate tokens: `--plate: #060A13`, `--plate-bezel: #1A2238` (dark theme) / `#2A3350` (light theme), `--plate-silk: #56647F` (silkscreen text, decorative, 10px mono caps), `--plate-screw: #222C44`.

Light theme LED dot field (hero background, not the plate): ink dots `rgba(31,94,255, α)` with α per level `[0.06, 0.09, 0.13, 0.18, 0.25, 0.34, 0.46, 0.60]`. No glow sprites on paper, since glow on paper reads as a smudge.

### 2.4 Typography

**Packages (all offline, variable, import Latin subsets only):**

| Role | Package | Import | Axes used |
|---|---|---|---|
| Display | `@fontsource-variable/bricolage-grotesque` | `@fontsource-variable/bricolage-grotesque/wght.css` | wght 500–800 (opsz auto) |
| UI / body | `@fontsource-variable/geist` | `@fontsource-variable/geist/wght.css` | wght 400–600 |
| Data / labels | `@fontsource-variable/geist-mono` | `@fontsource-variable/geist-mono/wght.css` | wght 400–500 |

Payload: about 45 KB + 30 KB + 28 KB (woff2, latin) ≈ **~105 KB total**. Preload only Geist (body) with `<link rel="preload" as="font" type="font/woff2" crossorigin>` on the latin file. Display and mono use `font-display: swap`. If the Geist fontsource packages are missing from the registry, fall back to the `geist` npm package (Vercel's, ships local woff2) or `@fontsource-variable/inter`. Both still work offline.

Global: `font-feature-settings: "ss01", "cv11"` for Geist. Use `font-variant-numeric: tabular-nums` on all counters and meta. Apply `text-wrap: balance` to headings and `text-wrap: pretty` to paragraphs.

**Type scale (fluid; viewport 360 → 1440):**

| Token | Family | Size (clamp) | Line-height | Weight | Tracking | Use |
|---|---|---|---|---|---|---|
| `display-xl` | Bricolage | `clamp(2.75rem, 1.6rem + 5.1vw, 6rem)` (44→96) | 0.95 | 720 | -0.035em | Hero headline |
| `display-l` | Bricolage | `clamp(2.25rem, 1.6rem + 2.9vw, 4rem)` (36→64) | 1.0 | 700 | -0.03em | Course name, 404 |
| `display-m` | Bricolage | `clamp(1.75rem, 1.4rem + 1.6vw, 2.75rem)` (28→44) | 1.05 | 680 | -0.025em | Chapter title, section heads |
| `title-l` | Bricolage | `clamp(1.375rem, 1.25rem + 0.55vw, 1.75rem)` (22→28) | 1.15 | 640 | -0.015em | Note view title, sheet title |
| `title-m` | Geist | `1.125rem` (18) | 1.35 | 580 | -0.01em | Card titles |
| `title-s` | Geist | `1rem` (16) | 1.4 | 560 | -0.005em | Chapter row title (mobile), list heads |
| `body-l` | Geist | `clamp(1.0625rem, 1rem + 0.3vw, 1.25rem)` (17→20) | 1.55 | 400 | 0 | Hero subline, course description |
| `body` | Geist | `1rem` (16) | 1.6 | 400 | 0 | Note body, forms (16px stops iOS zoom) |
| `body-s` | Geist | `0.875rem` (14) | 1.5 | 400 | 0 | Card descriptions, toasts |
| `label` | Geist | `0.875rem` (14) | 1.2 | 540 | 0 | Buttons, inputs labels |
| `meta` | Geist Mono | `0.8125rem` (13) | 1.4 | 450 | 0 | Dates, counts, file sizes |
| `caption` | Geist Mono | `0.6875rem` (11) | 1.3 | 500 | +0.08em, UPPERCASE | Silkscreen labels, stat labels, badges |
| `counter` | Geist Mono | `clamp(2rem, 1.5rem + 2.2vw, 3.5rem)` | 1 | 500 | -0.02em | Stat counters |

Body measure: max 68ch for note body text.

### 2.5 Spacing (4 px base)

`--space-0: 0` · `0.5: 2px` · `1: 4px` · `1.5: 6px` · `2: 8px` · `3: 12px` · `4: 16px` · `5: 20px` · `6: 24px` · `8: 32px` · `10: 40px` · `12: 48px` · `16: 64px` · `20: 80px` · `24: 96px` · `32: 128px`

These match Tailwind's default spacing (4px units), so use the stock utilities.

Layout:
- Container: `max-width: 1240px`. Gutters are 16px (<640), 24px (640–1023), 32px (≥1024).
- Section rhythm: 64px mobile / 96px desktop between major sections. Hero top padding 96px mobile / 144px desktop (below the 56/64px header).
- **Dot grid pitch: 24px desktop, 20px mobile.** The page background dot grid, the hero LED field and card internal alignment all snap to it. Card gaps are 16px (mobile 10px in masonry).

### 2.6 Radii

| Token | Value | Use |
|---|---|---|
| `--radius-xs` | 6px | Badges, kbd, small chips |
| `--radius-sm` | 10px | Inputs, icon buttons, thumbnails in strip |
| `--radius-md` | 14px | Buttons, image tiles, toasts |
| `--radius-lg` | 20px | Cards |
| `--radius-xl` | 28px | Sheet, modal panel, LED plate |
| `--radius-full` | 9999px | Pills (FAB, count badge), LED dots |

Nested radius rule: inner radius = outer radius − padding (e.g. a card of 20px with 6px padding gives a 14px cover image).

### 2.7 Shadows & glows

Dark theme (depth comes from borders + inner highlight; shadows are deep and soft):

```css
--hl-inset: inset 0 1px 0 0 rgba(255,255,255,0.05);        /* top edge highlight, all surfaces */
--shadow-1: 0 1px 2px rgba(0,0,0,0.45);
--shadow-2: 0 10px 30px -12px rgba(0,0,0,0.65);
--shadow-3: 0 30px 80px -20px rgba(0,0,0,0.75), 0 0 0 1px rgba(255,255,255,0.03);
--glow-sm:  0 0 12px 0 rgba(76,154,255,0.35);
--glow-md:  0 0 32px 0 rgba(76,154,255,0.28);
--glow-lg:  0 0 90px 10px rgba(30,91,255,0.22);
```

Light theme:

```css
--hl-inset: inset 0 1px 0 0 rgba(255,255,255,0.9);
--shadow-1: 0 1px 2px rgba(18,24,38,0.06), 0 0 0 1px rgba(18,24,38,0.04);
--shadow-2: 0 10px 28px -12px rgba(18,24,38,0.16);
--shadow-3: 0 34px 70px -24px rgba(18,24,38,0.28);
--glow-sm:  0 0 0 4px rgba(31,94,255,0.12);
--glow-md:  0 0 24px 0 rgba(31,94,255,0.14);
--glow-lg:  0 0 60px 0 rgba(31,94,255,0.10);
```

**Animating glows:** never animate `box-shadow`. Put the glow on a `::after`/child layer with the shadow baked in and animate only its `opacity`.

### 2.8 Blur / glass

Use glass in exactly three places:
1. **Header:** `background: color-mix(in oklab, var(--bg-0) 72%, transparent); backdrop-filter: blur(16px) saturate(1.4);` plus a 1px bottom border `--border-subtle`. The bottom border fades in after scrollY > 8.
2. **Badges over photos** (image count, "New"): `rgba(5,7,13,0.55)` + `blur(8px)` in both themes, because photos vary wildly in brightness.
3. **Modal scrim:** `var(--scrim)` + `blur(6px)`. On mobile, fall back to no blur on low-end devices (see perf §9).

Glass never goes on cards, where it hurts legibility and scroll performance.

**Grain:** a fixed full-viewport overlay of a 160×160 tiling noise PNG (generate once, commit to `/src/assets/grain.png`, ~6 KB), `opacity: 0.035` dark / `0.05` light, `mix-blend-mode: overlay`, `pointer-events: none`, z-index `--z-grain`. Static. It is never animated.

**Page dot grid:** on `body`, `background-image: radial-gradient(circle at 1px 1px, var(--grid-dot) 1px, transparent 0); background-size: 24px 24px;`, masked so it is strongest at the top: `mask-image: linear-gradient(to bottom, #000 0, #000 40vh, transparent 110vh)` (apply on a fixed pseudo layer, not body itself).

### 2.9 Z-index layers

| Token | Value | What |
|---|---|---|
| `--z-base` | 0 | Content |
| `--z-raised` | 10 | Hovered/tilted cards, dragged tile placeholder |
| `--z-fab` | 30 | Mobile upload FAB |
| `--z-header` | 40 | Sticky header |
| `--z-popover` | 50 | Tooltips, LED stats popover, menus |
| `--z-scrim` | 60 | Modal backdrop |
| `--z-modal` | 70 | Lightbox, upload sheet |
| `--z-dropzone` | 75 | Full-window drag-over overlay |
| `--z-toast` | 80 | Toasts |
| `--z-celebrate` | 90 | Celebration canvas |
| `--z-drag` | 100 | Actively dragged element |
| `--z-grain` | 110 | Grain overlay (pointer-events none) |

### 2.10 Breakpoints

Tailwind defaults plus one: `xs: 420px`, `sm: 640px`, `md: 768px`, `lg: 1024px`, `xl: 1280px`, `2xl: 1536px`. Design mobile-first at **375×812**, verify at 360, 768, 1024, 1440, 1920.

Input-mode queries are used as much as widths:
- `@media (hover: hover) and (pointer: fine)` enables tilt, spotlight and cursor-reactive effects.
- `@media (pointer: coarse)` sets 44×44px minimum targets and enables long-press-to-reorder.

---

## 3. Motion system

Setup: wrap the app in `<LazyMotion features={domMax} strict>` (layout + drag need `domMax`) and `<MotionConfig reducedMotion="user" transition={spring.ui}>`. Use `m.*` components, not `motion.*`, to keep the bundle small.

### 3.1 Duration tokens (CSS + TS)

| Token | ms | Use |
|---|---|---|
| `dur.instant` | 80 | Press feedback color, checkbox tick |
| `dur.fast` | 150 | Hover color/opacity, tooltip in |
| `dur.base` | 220 | Fades, small enter/exit, crossfades |
| `dur.slow` | 360 | Image blur-up, route fade-out |
| `dur.slower` | 560 | Hero word reveal (per word), large reveals |
| `dur.epic` | 900 | Upload arrow (matches firmware `UPLOAD_ANIM_MS`) |

### 3.2 Easing tokens

| Token | Value | Use |
|---|---|---|
| `ease.out` | `cubic-bezier(0.22, 1, 0.36, 1)` | Default for tween enters |
| `ease.outExpo` | `cubic-bezier(0.16, 1, 0.3, 1)` | Big reveals, blur-up, scrim |
| `ease.inOut` | `cubic-bezier(0.65, 0, 0.35, 1)` | Crossfades, theme transition |
| `ease.in` | `cubic-bezier(0.55, 0, 1, 0.45)` | Exits (always faster than enters) |
| `ease.led` | `steps(7, jump-end)` | LED glow fades only. It gives the discrete 8-level look |
| `ease.linear` | `linear` | Progress bars, shimmer, marquee |

### 3.3 Spring presets (`src/motion/springs.ts`)

```ts
export const spring = {
  press:   { type: "spring", stiffness: 700, damping: 40, mass: 0.6 }, // ζ≈0.98  tap scale
  snappy:  { type: "spring", stiffness: 500, damping: 36, mass: 0.8 }, // ζ≈0.90  toggles, tabs, crumbs
  ui:      { type: "spring", stiffness: 380, damping: 32, mass: 1   }, // ζ≈0.82  default, hover lift
  soft:    { type: "spring", stiffness: 210, damping: 26, mass: 1   }, // ζ≈0.90  page items entering
  morph:   { type: "spring", stiffness: 320, damping: 34, mass: 1   }, // ζ≈0.95  layoutId transitions
  sheet:   { type: "spring", stiffness: 420, damping: 40, mass: 1   }, // ζ≈0.98  sheets, lightbox dismiss
  pop:     { type: "spring", stiffness: 600, damping: 22, mass: 0.7 }, // ζ≈0.54  tile/badge pop-in (bouncy)
  tilt:    { type: "spring", stiffness: 260, damping: 20, mass: 0.5 }, // ζ≈0.88  3D card tilt follow
  swipe:   { type: "spring", stiffness: 300, damping: 30, mass: 1   }, // ζ≈0.87  gallery slide settle
} as const;
export const counterSpring = { stiffness: 90, damping: 22, mass: 1 };   // useSpring for count-up (~1.4s)
```

Rule: nothing user-facing overshoots more than about 4%, except `pop` (celebration, tile pop-in, badge increment). Those are the "delight" moments.

### 3.4 Stagger rules

- **Lists** (chapters, toasts): 40ms per item, max 10 items staggered. Item 11+ gets a delay of 400ms. `delayChildren: 80ms` after the page shell.
- **Grids** (courses, notes): diagonal wave, `delay = min((row + col) * 45ms, 450ms)`. Compute it from the index and the current column count.
- **Infinite-scroll appended pages:** stagger restarts at 0 for the new batch, 30ms per item.
- **Hero words:** 70ms per word, lines masked.
- **Never** stagger items that enter the viewport by scrolling after initial load, except through `whileInView` with `once: true` and `viewport={{ margin: "0px 0px -10% 0px" }}`.

### 3.5 Interaction patterns (apply everywhere)

| Pattern | Spec |
|---|---|
| **Enter (default item)** | `initial {opacity:0, y:14, filter:"blur(4px)"}` → `{opacity:1, y:0, filter:"blur(0px)"}` with `spring.soft`. The filter tweens separately at `dur.slow ease.outExpo`. Blur only on ≤ 12 items at once. |
| **Exit (default)** | `{opacity:0, y:-6}` in `dur.fast ease.in`. Exits are 60% of the enter duration. |
| **Hover (card)** | Lift `y:-3`, border → `--border`, glow layer opacity 0 → 1 (`dur.fast`). Tilt + spotlight on fine pointers (§6.2). |
| **Hover (button)** | Background color change `dur.fast`. Primary buttons also get an inner "LED sheen" sweep (§6.1). No scale on hover. |
| **Press** | `whileTap {scale: 0.97}` for buttons, `0.985` for cards, `spring.press`. Icon buttons `0.9`. |
| **Focus** | Ring appears instantly (no animation; a11y). Cards with focus get the same lift as hover. |
| **Drag (tile reorder)** | Picked tile: `scale:1.06, rotate: ±2°` (sign follows drag direction), `--shadow-3`, z `--z-drag`. Siblings reflow with `layout` + `spring.snappy`. On drop, settle with `spring.pop`. |
| **Drag (dismiss)** | Map offset to backdrop opacity and scale (§7.4). Release: velocity > 800px/s or offset > 120px dismisses, otherwise `spring.sheet` back. |
| **Disabled** | Opacity 0.45, no hover or press motion, `cursor: not-allowed`. |
| **Number change** | Rolling digits (§6.8), `spring.snappy` per digit. |
| **Error shake** | `x: [0, -6, 6, -4, 4, -2, 0]` over 360ms, `ease.inOut`. Reduced motion: none. The border color change carries the message. |

### 3.6 Route transition choreography

Wrap routes in `<AnimatePresence mode="popLayout" initial={false}>` keyed by the *page-level* pathname. Modal routes (note view, upload) do not change this key (§3.7). Each route has a depth: Home 0, Course 1, Chapter 2, Note 3. Direction is `sign(newDepth − oldDepth)`, stored in a ref and passed through `custom`.

**Forward (going deeper), 0–520ms:**

| t (ms) | What happens |
|---|---|
| 0 | Click. Clicked card does `whileTap` scale 0.985. Other cards on outgoing page fade to 0 over 160ms `ease.in` with `y:-8`. |
| 0 | Router commits. The outgoing page is popped out (`position:absolute`). The incoming page mounts with its shell at opacity 1. Shared elements (§3.8) start morphing from the old rects with `spring.morph` (~450ms settle). |
| 0 | Before paint (`useLayoutEffect`), the incoming page scrolls the window to top. |
| 80 | Incoming non-shared content staggers in (list/grid rules above) with `spring.soft`, from `y: 16`. |
| 120 | Header breadcrumbs: new crumb slides in from `x: 8, opacity 0` with `spring.snappy`; separator `›` fades in 60ms earlier. |
| ~520 | Settled. |

**Back (shallower):** mirror image. Outgoing content drops `y: 0 → 12`, fades 160ms. Incoming content enters from `y: -10`. Shared elements morph back into their cards. Scroll position of the shallower page is restored *before* paint (keep a `Map<pathname, scrollY>`) so the morph targets the right card.

**Unrelated jumps** (breadcrumb from Note to Home, 404): 220ms crossfade only, no shared elements.

The page wrapper itself never animates opacity on enter. Only its children do. If the wrapper faded, it would dim the shared element mid-morph.

### 3.7 Modal routes

- `/c/:slug/:chapterId/n/:noteId` and `/c/:slug/:chapterId/upload` render **over** the chapter page when navigated from it. Use the React Router "background location" pattern: `location.state.background` holds the chapter location. The grid stays mounted underneath, so the card → lightbox morph happens inside one tree, which is reliable.
- A direct deep-link (fresh load) renders the chapter page underneath anyway. Fetch the chapter first page in parallel so that closing lands on a real page with a natural morph back.
- Closing = `navigate(-1)` if we pushed it, otherwise `navigate(chapterPath, { replace: true })`.

### 3.8 Shared-element (layoutId) map

Put a `<LayoutGroup id="app">` at the root. IDs are namespaced strings.

| layoutId | From | To | Notes |
|---|---|---|---|
| `course:{slug}:surface` | Home course card background | Course page header panel | Morph radius 20 → 28 via `style={{borderRadius}}` (motion corrects radius distortion when it's set in `style`). |
| `course:{slug}:monogram` | Card LED monogram (13×8, dot 4px) | Course header LED monogram (dot 9px) | Canvas: re-render at target size; morph with `layout` on the wrapper, crossfade the canvas contents. |
| `course:{slug}:title` | Card title (`title-m`) | Header title (`display-l`) | Wrap text in `m.span layout="position"` + crossfade (don't scale text; use `layout="position"` + opacity swap of two sizes to avoid blurry glyphs). |
| `chapter:{id}:index` | Chapter row index "03" | Chapter page big index | Mono digits, same technique as title. |
| `chapter:{id}:title` | Chapter row title | Chapter page `display-m` title | `layout="position"`. |
| `chapter:{id}:cover` | Chapter row cover thumb (if any) | Chapter header strip background | Optional; skip if `cover_thumb_url` null. |
| `note:{id}:cover` | Note card cover image | Lightbox current image (image index 0 only) | The core morph. The cover uses `object-fit: cover` in the card and `contain` in the lightbox, so animate the **wrapper** and keep the image `object-fit: cover` inside a wrapper whose aspect ratio is the image's true ratio. The card crops via an outer clip. See §7.4. |
| `note:{id}:title` | Card title | Note panel title | `layout="position"`. |
| `note:{id}:count` | Card image-count badge | Lightbox counter "1 / 4" | Crossfade content. |
| `upload:surface` | "Upload notes" CTA button (header or FAB) | Upload sheet panel | Button morphs into sheet (radius 14 → 28). Label crossfades out in the first 100ms. |
| `upload:tile:{clientId}` | Upload preview tile #0 (cover) | `note:{newId}:cover` in the lightbox after success | Swap IDs at success time (§7.5). |

### 3.9 Scroll-linked effects (`useScroll` + `useTransform`, transform/opacity only)

| Where | Input range (scrollY px) | Output |
|---|---|---|
| Header | 0 → 24 | background alpha 0 → 0.72 (animate opacity of a bg layer), border opacity 0 → 1 |
| Header hide/show | direction | After scrollY > 240: scrolling down > 12px hides (`y: -100%`, `spring.snappy`), any upward scroll shows. Never hide while a popover is open or focus is inside. |
| Hero headline | 0 → 360 | `y: 0 → -48`, `opacity: 1 → 0` |
| Hero LED plate | 0 → 480 | `y: 0 → -80`, `scale: 1 → 0.9`, `rotateX: 0 → 14deg` (perspective 1000) |
| Hero LED field canvas | 0 → 600 | canvas `opacity: 1 → 0.25`. Pause rendering when fully out of view (IntersectionObserver). |
| Course page chapter rail | `useScroll({ target: listRef, offset: ["start 70%", "end 60%"] })` | Lit rail `scaleY: 0 → 1` (transform-origin top). Rail dots light at level 7 when their row passes 60% viewport, then settle to level 4 (`ease.led`). |
| Chapter page title | 0 → 140 | Big title `opacity 1 → 0`, `y 0 → -12`. Compact title in header crossfades in at 120px. |
| Mobile FAB | direction | Scroll down: collapses from pill "Upload notes" (width auto) to 56px circle (`layout`, `spring.snappy`, label opacity 0). Scroll up: expands. |

### 3.10 Reduced motion (`prefers-reduced-motion: reduce`)

`MotionConfig reducedMotion="user"` disables transform/layout animations automatically. Additionally:

- **All enters/exits** become opacity crossfades, `dur.base ease.inOut`. No y offset, no blur, no stagger (or 0ms).
- **Shared elements:** no morph. Crossfade old page out and new page in (200ms). The lightbox fades in over the grid.
- **Hero LED field:** render **one static frame** (t = 0, no cursor, no ripples). No RAF loop.
- **LEDMatrix:** keeps updating data (it's information, not decoration) but with no smooth column scroll or dot-by-dot growth. Heartbeat pixel stays. The upload arrow is replaced by a single 400ms brightness flash of the whole plate at level 5.
- **No** tilt, parallax, spotlight tracking, header hide-on-scroll or FAB collapse.
- **Celebration:** no particles or confetti. The matrix shows a static check glyph for 1.2s + success toast.
- **Counters:** show final numbers immediately.
- **Skeleton shimmer:** static dots at level 1.
- **Gallery swipe:** still works (it's direct manipulation, following the finger). Settle uses a 150ms tween instead of a spring. Pinch zoom unchanged.
- **Theme toggle:** instant, no circular reveal.

---

## 4. Screen specs

Common frame: sticky **Header** (§6.11) on top, **Footer** (§6.12) at the bottom, grain + dot grid behind everything.

### 4.1 Route table

| Path | Screen | Type |
|---|---|---|
| `/` | Home | page, depth 0 |
| `/c/:slug` | Course | page, depth 1 |
| `/c/:slug/:chapterId` | Chapter | page, depth 2 |
| `/c/:slug/:chapterId/n/:noteId?i=2` | Note view | modal route over Chapter, depth 3; `i` = 0-based image index, `replace` on change |
| `/c/:slug/:chapterId/upload` | Upload | modal route over Chapter |
| `*` | 404 | page |

Code-split `NoteView`/`Lightbox` and `UploadSheet` + celebration with `React.lazy`. Prefetch both chunks on first hover/touchstart of any note card / upload CTA.

### 4.2 Home

**Data:** `GET /api/courses` (each: slug, name, description, created_at, chapter_count, note_count, image_count, cover_thumb_url), `GET /api/stats` (shared poller).

**Mobile (375):**

```
┌──────────────────────────────────┐
│ [▦▦ notes]              [◐]      │  header 56px, mini LED in wordmark
├──────────────────────────────────┤
│ · · · · · · · · · · · · · · · ·  │  LED dot field canvas (full hero)
│ · · ·░░· · · · · · ·▒▒· · · · ·  │
│  EVERY NOTE FROM                 │  caption: "● LIVE ON THE LAN"
│  CLASS, IN ONE                   │  display-xl, 3 lines
│  PLACE.                          │
│  Photos of whiteboards, scans    │  body-l, text-2
│  and handwritten pages...        │
│                                  │
│  [ Browse courses ↓ ]            │  primary button, full width
│                                  │
│  ╭──────────────────────────╮    │  LED plate (hero size, 13×8)
│  │ ● ● ● ● ● ● ● ● ● ● ● ● ●│    │  width = 100% - 32px, max 340
│  │ ● ● ● ● ● ● ● ● ● ● ● ● ●│    │
│  │ ...   live bar graph  ...│    │
│  ╰──────────────────────────╯    │
│  UNO Q · 8×13 · 12 REQ/S · UP 3D │  caption silkscreen
├──────────────────────────────────┤
│ ┌───────┐┌───────┐               │  stats: 2×2 grid of counters
│ │  12   ││  48   │               │
│ │COURSES││CHAPTRS│               │
│ └───────┘└───────┘               │
│ ┌───────┐┌───────┐               │
│ │ 317   ││ 1,204 │               │
│ │ NOTES ││ PHOTOS│               │
│ └───────┘└───────┘               │
├──────────────────────────────────┤
│ Courses                    12    │  display-m + meta count
│ ┌──────────────────────────────┐ │  course card, 1 column
│ │ [LED halftone cover 16:9   ] │ │
│ │ [monogram]                   │ │
│ │ Linear Algebra               │ │
│ │ Vectors, matrices and...     │ │
│ │ 8 CH · 94 NOTES · 310 PH   → │ │
│ └──────────────────────────────┘ │
│ ...                              │
└──────────────────────────────────┘
```

**Desktop (1440):**

```
┌──────────────────────────────────────────────────────────────────────────┐
│ [▦▦ notes]                                      [▦ live 12/s]  [◐]       │ 64px
├──────────────────────────────────────────────────────────────────────────┤
│ · · · · · · · · · LED dot field canvas, full-bleed, 88vh max 860 · · · · │
│                                                                          │
│  ● LIVE ON THE LAN                          ╭────────────────────────╮   │
│  Every note                                 │ ● ● ● ● ● ● ● ● ● ● ● ●│   │
│  from class,                                │ ● ● ● ●  LED PLATE     │   │
│  in one place.                              │ ● ● ●  (dot 14, pitch  │   │
│                                             │   22 → 286×176 + pad)  │   │
│  Photos of whiteboards, scans and           ╰────────────────────────╯   │
│  handwritten pages — shared by students,    UNO Q · 8×13 · 12 REQ/S      │
│  served from a tiny board on your network.                               │
│  [ Browse courses ]  [ How it works ]         cols: 7 / 5 of 12          │
│                                                                          │
├──────────────────────────────────────────────────────────────────────────┤
│   12 COURSES   │   48 CHAPTERS   │   317 NOTES   │   1,204 PHOTOS        │ 1 row, dividers
├──────────────────────────────────────────────────────────────────────────┤
│ Courses                                                       12 total   │
│ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐                        │ 3 cols ≥1024
│ │ card         │ │ card         │ │ card         │                        │ 2 cols ≥640
│ └──────────────┘ └──────────────┘ └──────────────┘                        │ gap 16 / 24
└──────────────────────────────────────────────────────────────────────────┘
```

**Components:** `Hero` → `LEDField` (canvas), `HeroCopy` (`Eyebrow`, `RevealHeadline`, `Subline`, `ButtonRow`), `LEDPlate size="hero"`, `PlateCaption` · `StatsRow` → 4 × `Counter` · `CourseGrid` → `CourseCard[]` | `CourseGridSkeleton` | `EmptyCourses` | `ErrorPanel`.

**Hero choreography (first load, t from DOMContentLoaded):**
- 0ms: LED field fades from 0 over 600ms `ease.outExpo` while its noise field is already moving.
- 100ms: Eyebrow "● LIVE ON THE LAN". The dot is level 7 with the heartbeat, pulsing each time a stats poll arrives. Fades in.
- 180ms: headline lines reveal. Each word is in an `overflow:hidden` line wrapper, `y: 110% → 0`, `spring.soft`, 70ms per word.
- 520ms: subline fades up (y 10).
- 640ms: buttons fade up.
- 300ms: LED plate powers on. It rises from `y:24, rotateX: 18deg, opacity 0` with `spring.soft`, then plays the firmware **"starting" comet** for 840ms (one full ring rotation) before switching to the live dashboard. That's a "boot" moment and only happens once per session (`sessionStorage`).
- Stats counters count up when 40% in view.

**Course card grid:** diagonal-wave stagger. Loading shows 6 `CardSkeleton`s (3 on mobile).

**Empty state (0 courses):** a centered block in the grid area:
- LED plate `size="md"` running the firmware **"waiting" view** (dim dot sweeping the bottom row, 120ms per step) forever.
- Title (`title-l`): "No courses yet"
- Body: "Courses are created by the admin with the `notes-admin` terminal app. As soon as one exists, it'll appear here — no refresh needed." (Poll `/api/courses` every 15s while empty. When a course appears, it pops in with `spring.pop` and a toast.)
- No button. Optionally a mono "kbd"-style chip: `notes-admin → Courses → New`.

**Error state (courses fetch failed):** LED plate shows **blinking X** (firmware "down"). Title "Can't reach the board". Body "The server didn't answer. It might be restarting — this page will retry on its own." Buttons: `[Retry now]` (secondary). Auto-retry with backoff 2s, 4s, 8s, max 30s.

### 4.3 Course page

**Data:** `GET /api/courses/{slug}` → course + chapters (ordered, each with note_count, image_count, cover_thumb_url).

**Mobile:**

```
┌──────────────────────────────────┐
│ ‹ Home                     [◐]   │  header, back-crumb
├──────────────────────────────────┤
│ ╭──────────────────────────────╮ │  course:{slug}:surface (morphed card)
│ │ [LED monogram 13×8, dot 7px] │ │
│ │ LINEAR ALGEBRA               │ │  display-l
│ │ Vectors, matrices, and       │ │  body-l text-2
│ │ eigen-everything.            │ │
│ │ 8 CHAPTERS · 94 NOTES · 310 PH│ │  caption
│ ╰──────────────────────────────╯ │
│                                  │
│ ●┐ 01  Vectors                 › │  chapter row, rail on the left
│ ││     12 notes · 40 photos      │
│ ●┤ 02  Matrices                › │
│ ││     9 notes                   │
│ ○┤ 03  Determinants            › │  unlit rail dot (not yet scrolled)
│ ││     No notes yet — be first   │
│ ○┘ ...                           │
└──────────────────────────────────┘
```

**Desktop:**

```
┌──────────────────────────────────────────────────────────────────────────┐
│ [▦▦ notes]  Linear Algebra                           [▦ live]  [◐]       │
├──────────────────────────────────────────────────────────────────────────┤
│ ╭──────────────────────────────────────────────────────────────────────╮ │
│ │  [LED MONOGRAM          ]   Linear Algebra                            │ │  header panel, 2 cols
│ │  [ dot 9, pitch 14      ]   Vectors, matrices, and eigen-everything.  │ │  monogram 182×112
│ │  [ 182×112              ]   8 CHAPTERS · 94 NOTES · 310 PHOTOS        │ │
│ ╰──────────────────────────────────────────────────────────────────────╯ │
│                                                                          │
│   ●━┓  01   Vectors                         12 notes  [thumb][thumb] ›  │  rows 88px tall
│   ┃ ┃                                                                    │  rail x=24
│   ●━┫  02   Matrices                          9 notes  [thumb]        ›  │  max-width 880, centered
│   ┃ ┃                                                                    │
│   ○━┫  03   Determinants                  No notes yet                ›  │
│   ┃ ┃                                                                    │
└──────────────────────────────────────────────────────────────────────────┘
```

**The chapter rail ("signal timeline"):** a vertical 2px line of dots (a column of LED dots at 8px pitch, level 0) at the left, with one large 10px dot per chapter. A lit overlay (`--accent`, same dot pattern via mask) scales with scroll (§3.9). Each chapter's big dot lights to level 7, then decays to level 4 when reached, animated with `ease.led` over 280ms. That's the visual metaphor: signal travelling down the course.

**Chapter row anatomy:** `[rail dot] [index "01", mono 13 on mobile / display-m mono-ish 28 desktop, text-3] [title title-s/title-m] [meta: "12 notes · 40 photos" meta text-3] [cover thumb stack: up to 2 overlapping 40×40 rounded thumbs (desktop only), rotated -4° and +3°, like photos on a desk] [chevron]`.
- Hover: row bg → `--surface-1`, `x: 4` on title group (`spring.ui`), chevron `x: 2`. The thumb stack fans out (rotations to -8°/+7°, x offset ±6).
- Focus: ring on the row (row is an `<a>`).
- Zero notes: meta reads "No notes yet — be first" in `--text-3`, the rail dot stays level 2.

**States:** loading = header skeleton (monogram dots at level 1 shimmering) + 5 row skeletons. **404 course** → inline not-found panel: "This course isn't here anymore" + [Back to courses]. **No chapters:** rail with a single dim dot + "No chapters yet. The admin adds chapters with `notes-admin`."

### 4.4 Chapter page

**Data:** `GET /api/chapters/{id}?limit=24&offset=N` → chapter (title, position, total_notes) + notes page (oldest first). **Sort:** default **Newest first**. Recommended server tweak: support `?order=desc`. Until then, compute tail offsets: `offset = max(0, total − (page+1)·24)` and reverse each page client-side. Sort control: segmented `[Newest | Oldest]` (layoutId pill indicator `sort:pill`, `spring.snappy`).

**Mobile:**

```
┌──────────────────────────────────┐
│ ‹ Linear Algebra           [◐]   │
├──────────────────────────────────┤
│ CHAPTER 02                       │  caption, accent
│ Matrices                         │  display-m (fades into header on scroll)
│ 9 notes · 31 photos              │  meta
│ [Newest|Oldest]                  │  segmented, 36px
├──────────────────────────────────┤
│ ┌──────────┐ ┌──────────┐        │  2-col masonry, gap 10
│ │ [thumb ] │ │ [thumb ] │        │  aspect from thumb_w/h, clamped 3:4..4:3
│ │ [   ▦ 4] │ │ [      ] │        │  count badge glass, top-right
│ │ [  tall] │ │──────────│        │
│ │──────────│ │ Row ops  │        │
│ │ Gaussian │ │ Mia · 2h │        │  title-m 2 lines, meta 1 line
│ │ elim.    │ └──────────┘        │
│ │ Sam · 3d │ ┌──────────┐        │
│ └──────────┘ │  NEW     │        │
│   ...        │ ...      │        │
│         · · · loading · · ·      │  LED 13-dot loader row
│                                  │
│                ╭──────────────╮  │  FAB bottom-right: 16px + safe-area
│                │ ⇪ Upload notes│  │  56px tall, pill, accent fill, glow-md
│                ╰──────────────╯  │
└──────────────────────────────────┘
```

**Desktop:**

```
┌──────────────────────────────────────────────────────────────────────────┐
│ [▦▦ notes]  Linear Algebra › Matrices           [▦ live] [⇪ Upload] [◐] │  compact title after scroll
├──────────────────────────────────────────────────────────────────────────┤
│  CHAPTER 02                                                              │
│  Matrices                                          [Newest|Oldest]       │
│  9 notes · 31 photos                         [ ⇪ Upload notes ] (large)  │
├──────────────────────────────────────────────────────────────────────────┤
│ ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐                                      │  4 cols ≥1100, 5 ≥1440
│ │      │ │      │ │      │ │      │                                      │  3 cols ≥768
│ │      │ └──────┘ │      │ │      │                                      │  gap 16
│ └──────┘ ┌──────┐ └──────┘ └──────┘                                      │
│   ...                                                                    │
└──────────────────────────────────────────────────────────────────────────┘
```

**Masonry algorithm:** JS "shortest column" placement (keeps reading order stable when pages are appended). Card height = `colWidth / clamp(thumb_w/thumb_h, 0.75, 1.333)` + text block (measured as a fixed 76px: title max 2 lines + meta). Because aspect ratios come from the API, the layout is final **before** any image loads, so there is zero CLS. Recompute on resize (ResizeObserver on the grid, rAF-throttled). Items use `layout="position"` so a resize reflows smoothly (`spring.snappy`), but disable layout animation during active window resizing (debounce 150ms).

**Note card anatomy (`NoteCard`):**
- Surface `--surface-1`, radius 20, padding 6, border `--border-subtle`, `--hl-inset`.
- Cover: `ImageTile` (§6.9) of `images[0]`, radius 14. `srcset="{thumb_url} 300w, {url} 1600w"`, `sizes="(min-width:1440px) 260px, (min-width:1100px) 290px, (min-width:768px) 33vw, 50vw"`. On 2-col phones at DPR 3, browsers pick the 1600 version. That's fine on a LAN, and the thumb is still used as the blur-up placeholder.
- Badge top-right (inside cover, 8px inset): `▦ 4` (lucide `images` 12px + mono 12) if images > 1. Glass style.
- Badge top-left: `NEW` highlighter pill if `created_at` < 24h ago.
- Text block (padding 10px 8px 8px): title `title-m` (16px on mobile), 2-line clamp; meta row `meta text-3`: `{author_name ?? "Anonymous"} · {relative date}` with `Intl.RelativeTimeFormat` ("2h", "3d", then "Sep 12"). Full date in `title` attr / `<time dateTime>`.
- Body text is NOT shown on cards (keeps the grid visual); if the note has a body, show a tiny `align-left` icon 12px next to meta as a hint.
- Hover: lift −3, cover image `scale 1.04` inside its clip (`dur.slow ease.out`), spotlight border (§6.2). No tilt on note cards: tilt fights the masonry.
- Press: 0.985. Then the cover morphs into the lightbox.

**Paging:** IntersectionObserver sentinel with `rootMargin: 1200px 0px`. Fetch 24 at a time. While fetching, show a 13-dot "LED loader" row (§6.13) centered under the grid, plus 4 skeleton cards appended to the shortest columns. Always also render a visually-subtle `[Load more]` button under the sentinel for keyboard/AT users and as a fallback. End of list: `· · · · · · ●  That's all 42 notes  ● · · · · · ·` in meta text-3, with the dots lit left-to-right once (`ease.led`, 30ms per dot) on reveal.

**Upload CTA:** desktop = primary button `lg` in the header row (and a compact `sm` copy in the sticky header after scroll). Mobile = FAB. Both carry `layoutId="upload:surface"`, and only one is mounted at a time per breakpoint.

**Empty chapter:**

```
        ╭───────────────────────╮
        │  LED plate md: upload │   plays the firmware arrow animation on loop,
        │  arrow flying up      │   every 2.4s (900ms anim + 1500ms rest)
        ╰───────────────────────╯
          No notes here yet
   Be the first — snap a photo of your notes
   and everyone in Matrices can see them.
        [ ⇪ Upload the first note ]
```

**Full-window drop target (desktop):** dragging files anywhere over the chapter page shows an overlay (`--z-dropzone`): scrim at 60%, a 24px-inset rounded rect (radius 28) with a **marching-dots border** (SVG rect, `stroke-dasharray: 0 12`, `stroke-linecap: round`, stroke-width 4, stroke `--accent`, `stroke-dashoffset` animated linearly −24px per 600ms), centered text "Drop to add photos to **Matrices**" (`title-l`). Drop opens the upload sheet with the files already added.

**States:** loading (header skeleton + 8 masonry skeletons with plausible random aspect ratios seeded by index), error (inline panel + retry), 404 chapter (not-found panel with link to course).

### 4.5 Note view (lightbox)

**Data:** from the card that was clicked (instant), refreshed with `GET /api/notes/{id}` (note + images). On deep link: fetch note (render skeleton panel + blurred thumb once known).

**Desktop (≥1024) layout: stage + side panel:**

```
┌──────────────────────────────────────────────────────────────────────────┐
│ scrim (--scrim + blur 6px) over chapter page                             │
│ ┌───────────────────────────────────────────────────┐┌─────────────────┐ │
│ │ [×]                                     1 / 4     ││ Gaussian        │ │ panel 380px
│ │                                                   ││ elimination     │ │ surface-2, r28
│ │        ‹        [ current image, contain ]      › ││ title-l         │ │
│ │                                                   ││ Sam · Sep 12    │ │
│ │                                                   ││ ─────────────── │ │
│ │                                                   ││ body text,      │ │
│ │                                                   ││ body 16/1.6,    │ │
│ │ [−][+][⤢]                         [⇩ Download][↗] ││ scrolls         │ │
│ └───────────────────────────────────────────────────┘│ ─────────────── │ │
│   [t1][t2][t3][t4]   thumbnail strip, centered        │ 4 PHOTOS · 3.1MB│ │
│                                                       │ scan.png 1600×… │ │
│                                                       └─────────────────┘ │
└──────────────────────────────────────────────────────────────────────────┘
 inset 24px all sides; stage = flex 1; strip 64px tall under stage.
```

**Mobile layout: full-screen stage + draggable details sheet:**

```
┌──────────────────────────────────┐
│ [×]                     1 / 4 [⋯]│  top bar, gradient-to-transparent, 56px
│                                  │
│                                  │
│     [ image, contain, swipe ]    │
│                                  │
│                                  │
│ ● ○ ○ ○                          │  page dots (LED dots: current = level 7)
├──────────────────────────────────┤  details sheet, snap points:
│ ═══                              │   peek 132px / half 55% / full 92%
│ Gaussian elimination             │
│ Sam · 3 days ago · 4 photos      │
│ [t1][t2][t3][t4]                 │  strip lives in the sheet on mobile
│ Body text...                     │
└──────────────────────────────────┘
```

**Background:** stage background `#05070D` in both themes (photos need neutral dark). The scrim covers the page.

**Opening choreography (from card):**
1. 0ms: scrim fades 0 → 1 (`dur.slow ease.outExpo`).
2. 0ms: `note:{id}:cover` morphs from card rect to stage-fitted rect with `spring.morph`. During the morph it shows the **thumb** (already cached), so there's no network wait.
3. 0ms: the full 1600px image starts loading. When `img.decode()` resolves, it crossfades in over the thumb (`dur.slow`). The thumb underneath has `filter: blur(6px)` applied only *after* the morph ends (so the morph isn't blurred and then sharpened twice).
4. 140ms: side panel slides in `x: 24 → 0` + fade (`spring.soft`). Its children stagger 40ms. On mobile the sheet rises to "peek" with `spring.sheet`.
5. 200ms: controls and strip fade in.
6. Focus moves to the close button. Focus is trapped. `inert` goes on the page underneath.

Opening at image index `i > 0` (deep link or strip): no cover morph. The stage fades + scales from 0.96.

**Closing:** reverse. If the current index ≠ 0, the current image shrinks to 0.9 + fades while the card cover fades in (no false morph). The URL goes back.

**Gallery mechanics:**
- **Slides** are a horizontal track of all images, and only current ±1 are mounted with `<img>` (others are placeholders).
- **Swipe (touch/mouse drag):** `drag="x"` on the track when zoom = 1, `dragElastic: 0.18` at the ends, `dragMomentum: false`. Release: go to next/prev if `|offset.x| > 22% width` or `|velocity.x| > 500px/s`. Settle with `spring.swipe`.
- **Keyboard:** `←/→` prev/next · `Home/End` first/last · `Esc` close (first Esc resets zoom if zoomed) · `+`/`=` zoom in · `-` zoom out · `0` reset · `D` download · `I` or `Tab` into panel. Show a tiny `?` shortcut hint popover on desktop.
- **Zoom:** scale range 1–4. **Double-tap/double-click** toggles 1 ↔ 2.5 anchored at the tap point (`spring.snappy`). **Pinch:** track two pointers with Pointer Events (`touch-action: none` on stage). `scale = startScale × dist/startDist`, anchor at the midpoint, pan follows the midpoint. Clamp pan so image edges can't pass stage edges (rubber-band 0.3× beyond). **Wheel:** `ctrl/⌘ + wheel` or trackpad pinch (`wheel` with `ctrlKey`) zooms, plain wheel on the stage does nothing (or pans when zoomed). When zoomed, horizontal drag pans instead of swiping. Release below 1.05 snaps back to 1.
- **Swipe down to dismiss (touch, zoom = 1):** `drag="y"` on the stage. Map `y` → image `scale: 1 → 0.82` (0 → 300px), scrim opacity `1 → 0.2`, panel/sheet opacity `1 → 0`. Release: dismiss if `y > 120` or `vy > 800`. When dismissed, morph back into the card (index 0) or fade.
- **Thumbnail strip:** 56×56 (desktop 64×64) thumbs, radius 10, gap 8, scroll-snap. Current thumb: 2px `--accent` ring + level-7 LED dot under it, with the ring moving between thumbs via `layoutId="strip:indicator"` (`spring.snappy`). Non-current thumbs opacity 0.55 → 1 on hover. Auto-scroll current into view (`scrollIntoView({inline:"center", behavior: reduced ? "auto" : "smooth"})`).
- **Prev/next arrows (desktop):** 44px icon buttons, glass, vertically centered, appear on stage hover (`opacity 0 → 1`, `dur.fast`) and always when focused. Disabled at ends (opacity 0.3).
- **Actions:** `[Download]` = `<a href={url} download={original_filename ?? "note-{i}.jpg"}>` (the 1600px JPEG, which is the "original" as stored). `[Open]` = `target="_blank"` to the image URL. Mobile: both in the `⋯` menu + `navigator.share({ files })` if available ("Share").
- **Image meta (panel footer):** `original_filename`, `width×height`, `size_bytes` humanised (`Intl.NumberFormat` + KB/MB), mono `meta`.
- **URL:** `?i=` updates with `replace: true` on every slide change, so links deep-link to the exact photo. Title: `document.title = "{note} — {chapter} · notes"`.

**States:** image load failure: tile shows `image-off` icon + "Couldn't load this photo" + [Retry]. Note 404 (deleted): lightbox shows a centered message "This note was removed" + [Back to chapter], with the LED X icon.

### 4.6 Upload flow

**Decision: a route-backed, morphing sheet over the chapter page (not a separate page, not a wizard).**
Why:
1. **Context:** you're adding to *this chapter*. Keeping the grid visible behind the scrim makes that obvious, and on success the new note visibly lands in that grid.
2. **Continuity:** the CTA morphs into the sheet and the sheet morphs into the new note. A separate page would break that chain.
3. **Resilience:** the route (`/upload`) survives refresh (files don't, but the form text is restored from `sessionStorage`), and the back button closes it. That matches Android back-gesture expectations.
4. **One scrolling form:** photos first, then title, then optional fields. A wizard would add taps on a phone for a 3-field form.

**Sizing:** mobile (< 640) = bottom sheet with 12px top gap from safe-area, radius 28 on top corners, full height. The keyboard pushes content: use `height: 100dvh` and `interactive-widget=resizes-content` in the viewport meta. Tablet/desktop = centered panel, `width: min(720px, 100vw − 48px)`, `max-height: min(88dvh, 860px)`, radius 28, `--shadow-3`, `--surface-2`.

**Layout (mobile):**

```
┌──────────────────────────────────┐
│ ═══  (grabber)                   │
│ Upload to Matrices          [×]  │  title-l + chapter in accent
├──────────────────────────────────┤
│ ┌──────────────────────────────┐ │  DROPZONE (empty): 200px tall
│ │   · · · · · · · · · · · ·    │ │  marching-dots border on drag-over
│ │   ⇪  Add photos of your notes│ │
│ │   JPEG, PNG, WebP or GIF ·   │ │  meta text-3
│ │   up to 10 photos, 10 MB each│ │
│ │ [📷 Take photo] [🖼 Choose]  │ │  two buttons, 50/50
│ └──────────────────────────────┘ │
│                                  │
│ PHOTOS 3/10          drag to sort│  caption
│ [COVER][ t2 ][ t3 ][ + ]  →      │  filmstrip, 96px tiles, horiz scroll
│                                  │
│ Title *                          │
│ [ Gaussian elimination        ]  │  Input
│                       22 / 200   │  counter appears > 150
│ Notes (optional)                 │
│ [ textarea, 4 rows, autogrow  ]  │
│ Your name (optional)             │
│ [ Anonymous                   ]  │  remembered in localStorage
├──────────────────────────────────┤
│ [       Post note  (3 photos)  ] │  sticky footer, primary lg, full width
└──────────────────────────────────┘
```

Once ≥1 photo is added, the big dropzone collapses (`layout`, `spring.snappy`) into the filmstrip's trailing `[+]` tile, which keeps both "camera" and "files" options through a small popover on tap. On desktop, the dropzone stays as a slim 72px strip above the filmstrip.

**Inputs for picking:**
- `Take photo`: `<input type="file" accept="image/*" capture="environment">` (single). Shown only on `pointer: coarse`.
- `Choose photos`: `<input type="file" accept="image/jpeg,image/png,image/webp,image/gif" multiple>`. Listing concrete types makes iOS Safari transcode HEIC → JPEG automatically.
- Paste: `Ctrl/⌘+V` of images while the sheet is open adds them (desktop nicety).

**Previews (`UploadTile`):** `URL.createObjectURL(file)` (revoke on remove/unmount). Decode off-thread with `createImageBitmap` for a 192px preview canvas when files are > 4 MB, so large camera JPEGs don't jank the main thread. Tile 96×96 (mobile) / 120×120 (desktop), radius 14, `object-fit: cover`.
- Enter: `scale 0.6 → 1, opacity 0 → 1, rotate: ±3° → 0` with `spring.pop`, 50ms stagger for batch adds.
- Remove: `×` button (24px visual, 44px hit area) top-right. Exit `scale 0.6, opacity 0` in `dur.fast`, and siblings close the gap via `layout`.
- First tile shows a `COVER` caption badge (highlighter). Moving another tile to position 0 transfers the badge via `layoutId="upload:coverBadge"`.
- Size label bottom-left on hover/focus: "3.2 MB" mono 11.

**Reorder:** `Reorder.Group axis="x" values={files} onReorder={setFiles}` as a horizontal filmstrip on all breakpoints (1D keeps it native to motion, no extra deps). Touch: **long-press 250ms** to lift (use `useDragControls`, `dragListener={false}`, start controls on a timer; cancel if the pointer moves > 8px first, so horizontal scrolling still works). Haptic `navigator.vibrate?.(8)` on lift. Mouse: drag immediately. Auto-scroll the strip when dragging within 48px of its edges. **Keyboard:** tiles are focusable. `←/→` moves focus, `Alt+←/→` (or `Ctrl`) moves the tile, `Delete/Backspace` removes. An `aria-live="polite"` region announces "Photo 2 moved to position 1".

**Client-side validation (before any network):**

| Rule | Where it shows | Copy |
|---|---|---|
| 0 photos on submit | Dropzone border → `--danger`, shake, message under it | "Add at least one photo." |
| > 10 photos | Extra files are not added; toast (warning) | "That's more than 10 — kept the first {n}." |
| File > 10 MB | Tile gets danger ring + `!` badge; not uploadable until removed | "Over 10 MB — too big for the board." |
| Wrong type (by MIME/extension) | Tile danger state | "Not a JPEG, PNG, WebP or GIF." |
| Total > 60 MB | Banner above footer; submit disabled | "Together these are {x} MB — the limit is 60 MB per note. Remove a few." |
| Title empty on submit | Input error | "Give it a title so classmates can find it." |
| Title > 200 chars | Counter turns danger at 200, input prevents more | — |

Validate title on blur and on submit (never while typing the first time). After the first error, re-validate on change. Submit focuses the first invalid field and scrolls it into view.

**Uploading state:**
- Submit button morphs (same element, `layout`) into a **progress bar**: height 52px, accent fill as a `scaleX` child (transform-origin left), label crossfades to "Uploading 2 of 4 · 63%" (mono numbers, tabular).
- **Per-file feel:** from XHR `upload.onprogress` (`loaded/total`), compute each file's cumulative byte range in form order: `fileProgress_i = clamp((loaded − start_i) / size_i, 0, 1)` (ignore multipart overhead, since it's small). Each tile gets an overlay: dark 45% veil that lifts as `scaleY` from bottom, and a **13-dot LED bar** along its bottom edge that lights left→right with the file's progress (level 2 → level 7 when that file completes, with a `pop` on the tile at completion).
- The header/hero LED widgets play the **upload arrow** animation for local feedback, before the server even reports it in `/api/stats`.
- Fields are `disabled` (opacity 0.6), the close button becomes "Cancel upload" (aborts the XHR with confirmation), and the sheet can't be drag-dismissed.
- After 100% bytes → **"Processing on the board…"** (the server is resizing). The bar becomes indeterminate: a 13-dot LED row showing the firmware **comet** sweeping (70ms per step). If processing takes > 8s, add sub-copy "Big photos take a moment on a tiny computer."

**Success celebration ("LED burst"), total ~1.9s:**

| t (ms) | Beat |
|---|---|
| 0 | Response 201 with NoteDto. Button label → `✓ Posted`, fill → `--success`. Brief `pop` scale 1 → 1.04 → 1. |
| 0–900 | A 13×8 LED plate (`size="lg"`, centered in the sheet, fades in over the form at 90% opacity) plays the **firmware upload arrow** flying up (identical to `view_upload`: arrow bitmap `[0x04,0x0e,0x15,0x04,0x04]`, 5 wide at x=4, top from 8 to −5 over 900ms). |
| 900 | **Burst:** on the celebration canvas (`--z-celebrate`, full viewport), every lit dot of the final arrow frame plus 64 extra particles spawn at the plate's center. Particles: 70% LED blue `#4C9AFF`/`#DCEBFF`, 20% highlighter `#FFD84A`, 10% white. Shape: rounded squares 5–8px with glow sprite (dark theme) / flat (light theme). Velocity: angle uniform 0–2π biased upward (`angle = −π/2 + (rand−0.5)·1.8π`), speed 380–900 px/s. Gravity 1400 px/s². Drag 0.985 per frame. Spin ±6 rad/s. Life 900–1400ms, fade alpha in last 40% with **quantized steps** (`round(alpha·7)/7`), so it flickers out like LEDs. Cap 120 particles. Stop the RAF loop when all are dead. |
| 1000 | Sheet contents fade out (`dur.base`). The cover tile's layoutId switches to `note:{newId}:cover`. Navigate `replace` to `/c/:slug/:chapterId/n/:newId`. The sheet surface cross-morphs into the lightbox stage (`spring.morph`), and the cover flies into the stage. |
| 1100 | Behind the scrim, the new `NoteCard` is inserted at the top of the grid (in Newest sort) with `spring.pop` and a highlighter ring pulsing twice (`opacity` of ring layer 0 → 1 → 0, 2×, 600ms each). Counters in the header/footer increment with rolling digits. |
| 1300 | Toast: "Posted to Matrices" with `[Copy link]` action. |

Reduced motion: skip the arrow and particles. The plate shows a static check glyph (3×5-style check drawn in dots) for 1200ms, then a crossfade to the note view.

The `author_name` is saved to `localStorage["notes.author"]` on success. The title/body draft in `sessionStorage` is cleared.

**Server error states (form and files always preserved; nothing is lost):**

Parse the error body `{"error": string}`. Also detect per-image errors with `/^image (\d+) \((.+?)\): (.+)$/i` (1-based index), which point at a tile.

| Status | Detection | Presentation | Copy |
|---|---|---|---|
| 400 (per-image) | regex match | That tile: danger ring, shake, `!` badge, message popover under strip. Button returns to "Post note". Focus moves to that tile. | "**scan.png** couldn't be read — it may be corrupt or an unsupported format. Remove it or pick another." |
| 400 (other) | no match | Inline banner (danger) at top of form with the server message verbatim, sentence-cased | "Something's not right: {message}" |
| 404 | status | Whole sheet → state panel with LED X | Title "This chapter is gone". Body "It was removed while you were uploading. Your photos are still here — pick another chapter to post them." Buttons `[Back to course]` |
| 413 | status | Banner (danger) + the 2 largest tiles get a size highlight | "That's more than the board accepts in one go. Remove a photo or two, or use smaller ones (max 10 MB each, 60 MB total)." |
| 415 | status (+regex for tile if present) | Tile or banner | "The board only takes JPEG, PNG, WebP or GIF photos." |
| 429 | status; read `Retry-After` / `x-ratelimit-after` header (seconds) if present, else 30s | Banner (warning) with a **countdown ring** around the retry button (SVG circle `pathLength` 1 → 0, linear). The button reads "Try again in 12s", enables at 0 with a `pop`. | "Easy there — too many uploads in a short time. You can try again in {n}s." |
| 507 | status | Banner (warning, persistent) + LED plate in sheet shows firmware **warning view** (top row blinking 500ms) | "The board's storage is full, so it can't take new photos right now. Let your admin know." Button `[Close]` (no retry) |
| 5xx / network / abort-timeout | `xhr.onerror`, status ≥ 500, or 120s timeout | Banner (danger) with `[Retry]` | "Couldn't reach the board. Check you're on the same network and try again." (5xx: "The board hit a snag. Try again in a moment.") |
| Offline before submit | `navigator.onLine === false` | Submit disabled, inline hint | "You're offline." |

Banners: enter `height auto` via `layout` + opacity, icon (lucide `triangle-alert` / `hard-drive` / `wifi-off` / `clock`) with `spring.pop`, radius 14, `--danger-soft` / warning-soft bg, 1px border in status color at 40% alpha. `role="alert"`.

**Dismiss rules:** `Esc`, `×`, backdrop click (desktop), drag down on the grabber (mobile, only when scrolled to top). If the form is dirty, the sheet doesn't close. It shows an inline confirm bar sliding up from the footer: "Discard 3 photos and your title?" `[Keep editing] [Discard]`.

### 4.7 404 page

```
            ╭──────────────────────────────╮
            │  ● ● ● ● ● ● ● ● ● ● ● ● ●   │  LED plate "lg"
            │  ● ███ ● ███ ● ███ ● ● ● ●   │  "404" in the firmware 3×5 font
            │  ●  (x=1..11, y=1..5)        │  (3 glyphs × 4 cols − 1 = 11 cols fit exactly)
            ╰──────────────────────────────╯
                 This page isn't on the board.
         The link may be old, or the note was removed.
              [ Back to courses ]  [ Go back ]
```

Behavior: the plate first scrolls "NOT FOUND" once (firmware text scroll, 70ms/step), then settles on "404". Every 6s a single glitch: for 2 frames (120ms), the "404" is replaced by the firmware X. Reduced motion: static "404".

### 4.8 Global loading (first boot)

Before React mounts, `index.html` contains an inline SVG 13×8 dot grid (no JS, pure CSS keyframes) running a simple comet on the ring positions from firmware `view_starting`, centered on `--bg-0`. React replaces it. It stays visible only if the bundle takes > 300ms (use `animation-delay: 300ms` on its opacity).

### 4.9 Hero background: "LED Field" (canvas2D)

**Goal:** a full-bleed field of dots on the 24px grid (20px on mobile) that looks like a huge, dim LED panel. Soft "clouds" of light drift over it, a spotlight follows the cursor, and **every real request sends a ripple out of the LED plate**.

**Setup:**
- `<canvas>` absolutely fills the hero, `aria-hidden`, `pointer-events: none` (listen to pointer on the hero section).
- DPR = `min(devicePixelRatio, 2)` (1.5 on `pointer: coarse` devices with `navigator.hardwareConcurrency ≤ 4`).
- Grid: `cols = ceil(W / pitch) + 1`, `rows = ceil(H / pitch) + 1`, offset so dots align with the body dot grid. Desktop 1440×860 at 24px ≈ 2,200 dots. Mobile 375×760 at 20px ≈ 740 dots.
- **Pre-render 8 sprites** (one per level 0–7) to an offscreen canvas once per theme/DPR. Each sprite is a core circle (radius 1.1px @ L0 … 2.2px @ L7, color `--led-N`) plus, for L4–L7, a radial-gradient glow (radius per §2.3). Light theme uses ink-alpha dots, no glow. Draw with `drawImage(sprite, x − s/2, y − s/2)`. There are no per-dot gradients or `shadowBlur` at runtime.

**Per frame (t in seconds):**

```
for each dot (i, j) at (x, y):
  // 1. drifting clouds: cheap layered sines (no noise library)
  n  = 0.50 * sin(x*0.0090 + t*0.33) * cos(y*0.0110 - t*0.21)
     + 0.35 * sin((x + y)*0.0052 + t*0.47)
     + 0.15 * cos(x*0.0170 - y*0.0080 - t*0.62)          // n ∈ [-1, 1]
  v  = smoothstep(0.35, 1.0, n) * 0.55                    // mostly dark, sparse clouds

  // 2. cursor spotlight (fine pointers only; eased pointer position, lerp 0.12/frame)
  d2 = (x - px)^2 + (y - py)^2
  v += 0.85 * exp(-d2 / (2 * 110^2)) * pointerPresence   // presence eases 0↔1 on enter/leave

  // 3. request ripples (see below)
  for each ripple r: 
      dist = hypot(x - r.x, y - r.y)
      band = 1 - abs(dist - r.radius) / 48                 // 48px wide ring
      if band > 0: v += band * r.strength

  // 4. vignette: fade toward bottom and far edges so copy stays readable
  v *= mask(x, y)   // = smoothstep(0, 0.25, y/H inverted from bottom) * edge falloff;
                    //   also × 0.45 inside the headline's bounding box (+24px padding)

  level = min(7, floor(clamp(v, 0, 1) * 8))
  draw sprite[level]
```

- **Ripples:** subscribe to the stats store. When a new second arrives with `requests[12] = n > 0`, spawn `min(3, ceil(log2(n+1)))` ripples from the LED plate's center (in canvas coordinates, re-read on resize/scroll), staggered 120ms, `strength = 0.6`, radius grows at 520 px/s, strength decays `× 0.965` per frame, removed when radius > diagonal. When `uploads[12] > 0`, also spawn one **"upload column"**: a vertical band 3 dots wide at a random column that lights from bottom to top over 900ms (echo of the arrow). Max 6 live ripples.
- **Frame budget:** target 60fps desktop. On `pointer: coarse` run at 30fps (skip every other rAF). Draw only when something changes; the clouds always change, so skip frames based on budget. Measure: if the average frame time over 60 frames > 12ms, drop to 30fps and turn off glow sprites for L4–L5.
- **Lifecycle:** pause when the hero is out of view (IntersectionObserver, threshold 0), when `document.hidden`, and when a modal is open. Resize with ResizeObserver (debounced 100ms, rebuild grid).
- **Fallback (no canvas / low-power / reduced motion / `saveData`):** CSS-only. The hero gets a `radial-gradient` dot pattern at 24px using `--led-1` plus two large blurred blobs (`--accent-deep` at 18% alpha, 520px, `filter: blur(80px)`) that drift using a 24s `transform` keyframe loop (disabled under reduced motion, where it's static).

---

## 5. LED data & the stats store

### 5.1 Poller (`useStats` via `useSyncExternalStore`)

- One module-level store polls `GET /api/stats` every **1000ms**, aligned: after each response, schedule the next at `1000 − (Date.now() % 1000) + 50`. Use `fetch` with `cache: "no-store"` and an AbortController timeout of 2500ms.
- Pause when `document.hidden`. On resume, fetch immediately.
- Keep `lastOkAt`. **Down** = no successful response for > 5000ms (identical to firmware `DOWN_AFTER_MS`).
- Only components that are mounted subscribe. The poller starts on first subscriber and stops at zero.
- Expose `{ stats, state: "waiting" | "ok" | "warning" | "error" | "down", lastOkAt, tick }` where `tick` increments per fresh response (drives heartbeat/ripples).

### 5.2 Mapping to firmware views

| Store state | Firmware view reproduced |
|---|---|
| `waiting` (no response yet) | dim dot (L2) sweeping the bottom row, ping-pong, 120ms/step |
| boot (first mount per session, hero only) | "starting" comet: 12 ring points (copy `ring[]` from `mcu/notes-matrix/src/view.c`), head at L7, 4-dot tail L6…L3, 70ms/step |
| `ok` | dashboard: bar graph + upload tops + heartbeat |
| `warning` | dashboard + top row (y=0) all L7, blinking 500ms on/off |
| `error` | solid X: `(3+i, i)` and `(10−i, i)` for i in 0..7 at L7 |
| `down` | X blinking 500ms |
| upload event (`uploads[12] > 0` newly, or local upload in progress) | arrow flying up for 900ms, then back to dashboard |
| text (`LEDMatrix text="..."`) | 3×5 font scroll at 70ms/step from x=13 until off-screen, 4px advance per glyph, y=1..5 |

**Bar height (identical to firmware `bar_height`):** `n = 0 → 0`; else `h = 1; while (n > 1 && h < 7) { n >>= 1; h++ }`. Bars are L2 from the bottom row up. If `uploads[x] > 0`, the bar's top dot (or the bottom dot if h = 0) is L7. The **heartbeat** is at `(12, 0)`, fading L7 → L0 linearly over 400ms after each fresh response (quantized to levels, so it steps).

Copy the **3×5 font table verbatim** from `mcu/notes-matrix/src/view.c` (`font[]`: 0–9, A–Z, `. : - / ! %`) into `src/led/font.ts`. Unknown chars render blank. Lowercase maps to uppercase.

### 5.3 Web-only refinements (off under reduced motion)

- **Smooth scroll of history:** when a new second arrives, the 13 bar columns slide left by one pitch over 320ms (`ease.out`). Render with a fractional x-offset in the canvas, then snap. The new rightmost bar **grows dot by dot** from the bottom (35ms per dot, `ease.led`).
- **Dot persistence ("phosphor"):** when a dot's level drops, it decays one level per 45ms rather than instantly, like a slow LED. Rises are instant.
- **Hover popover** on plate/mini matrix (desktop hover, or tap on mobile header widget): card with `Live from the board` caption, `requests now: 14/s`, `peak (13s): 32/s`, `uploads (13s): 3 photos`, `up 2d 5h 12m`, status pill, and a legend: "Each column is one second. Height is requests (log scale). Bright tops are uploads." Opens with `spring.snappy`, `scale 0.96 → 1`, origin toward the trigger.

---

## 6. Component inventory

### 6.1 Button

Anatomy: `[leading icon 18] [label] [trailing icon 16 | kbd | spinner]`, gap 8, `label` type.

| Size | Height | Pad-x | Radius | Icon |
|---|---|---|---|---|
| `sm` | 36 | 12 | 10 | 16 |
| `md` | 44 | 16 | 14 | 18 |
| `lg` | 52 | 22 | 14 | 20 |
| `icon` | 44×44 (sm 36×36) | — | 12 | 20 |

| Variant | Rest | Hover | Pressed | Focus | Disabled |
|---|---|---|---|---|---|
| **primary** | bg `--accent`, text `--accent-fg`, `--hl-inset`, glow layer (glow-sm) opacity 0.6 (dark only) | bg `--accent-hover`; **LED sheen**: a 40%-wide diagonal highlight gradient (white 18%) sweeps `x: −120% → 220%` once, 700ms `ease.out`; glow opacity 1 | scale 0.97 (`spring.press`), glow 0.4 | 2px `--focus` ring, offset 2px + 4px `--accent-soft` halo | opacity 0.45, no sheen |
| **secondary** | bg `--surface-2`, 1px `--border`, text `--text-1` | bg `--surface-3`, border `--border-strong` | scale 0.97 | ring | 0.45 |
| **ghost** | transparent, text `--text-2` | bg `--surface-3` at 70%, text `--text-1` | scale 0.97 | ring | 0.45 |
| **icon** | ghost or glass (over photos: `rgba(5,7,13,.55)` + blur 8, icon white) | as ghost / glass 0.7 | scale 0.9 | ring | 0.3 |
| **danger** | text `--danger`, ghost style; confirm uses bg `--danger` text white | — | — | ring | — |

Loading: the label crossfades to a 5-dot LED loader (dots 4px, L2 → L7 comet, 70ms/step), and the width is locked (measure before swap) so the button doesn't jump. Always `<button type="button">` unless it submits a form.

### 6.2 Cards

**CourseCard** (Home):
- Surface `--surface-1`, radius 20, border `--border-subtle`, padding 6, `--shadow-1`.
- **Cover (16:9, radius 14):** if `cover_thumb_url`, the **LED halftone** treatment. Two stacked `<img>`: (a) the "halftone" layer with `filter: grayscale(1) contrast(1.15) brightness(0.9)` inside a wrapper with `mask-image: radial-gradient(circle, #000 1.6px, transparent 2.1px); mask-size: 6px 6px;` over a background of `--led-2` with the image layer `mix-blend-mode: screen` (dark) / `multiply` over `#dfe6f5` (light), so the photo reads as LED dots; (b) the true-color photo at `opacity: 0`. Hover/focus: (b) → opacity 1 (`dur.slow ease.outExpo`) and scale 1.04. That's the "reveal". If there's no cover, show the **LED monogram** large on a plate-colored panel.
- **Monogram chip:** 13×8 LED matrix (`size="xs"`, dot 3px, pitch 5px → 65×40), overlapping the cover's bottom-left by 20px, on a plate with radius 10, `--shadow-2`.
- Body (padding 14px 10px 10px): name `title-m` 18px, description `body-s text-2` 2-line clamp, footer row `caption text-3`: `8 CH · 94 NOTES · 310 PHOTOS` + trailing `arrow-up-right` 16px that moves `x:2,y:−2` on hover.
- **Hover (fine pointer):** lift −4. **3D tilt:** `rotateX = −(py − 0.5) × 7deg`, `rotateY = (px − 0.5) × 9deg` via motion values + `spring.tilt`, `transformPerspective: 900`, reset on leave. **Spotlight:** a `::before` layer `radial-gradient(360px circle at var(--mx) var(--my), rgba(76,154,255,0.16), transparent 45%)` (light: 0.08) with opacity 0 → 1. A **spotlight border** is a second layer masked to a 1px ring (`mask: linear-gradient(#000 0 0) content-box exclude, linear-gradient(#000 0 0)`) with the same gradient at 0.6 alpha. `--mx/--my` are set via `el.style.setProperty` in `pointermove` (rAF-throttled, no React state). The monogram does a **scanline**: rows light to L7 top→bottom, 40ms/row, then back to rest.
- Focus-visible: same as hover minus tilt + focus ring on the card link.
- Whole card is one `<a>` (stretched link). There are no nested interactive elements.

**ChapterRow** (Course page): see §4.3.

**NoteCard** (Chapter page): see §4.4.

**StatCard** (Home stats): mobile 2×2 cards (`--surface-1`, radius 20, padding 16, counter + caption label), desktop a single row with 1px `--border-subtle` vertical dividers, no card chrome.

**CardSkeleton**: same geometry, surface `--surface-1`, cover area filled with the **dot shimmer** (§6.13), text lines as 10px-tall rounded bars (`--surface-3`) at 70% / 45% width.

### 6.3 Badge

Height 22, pad-x 8, radius 6 (`pill` variant radius full), `caption` type (11px mono caps, +0.08em).

| Variant | Style |
|---|---|
| `neutral` | bg `--surface-3`, text `--text-2` |
| `accent` | bg `--accent-soft`, text `--accent` |
| `new` | bg `--highlighter`, text `#1A1400`. Enter with `spring.pop` from scale 0.6 |
| `glass` | over images: `rgba(5,7,13,0.55)` + blur 8, text white, icon 12px |
| `status` | leading 6px LED dot in status color (ok = accent with heartbeat, warning = amber blinking 500ms `steps(1)`, error = danger) |
| `cover` | highlighter, used on first upload tile |

Count changes animate via rolling digits (§6.8).

### 6.4 Breadcrumbs

- Desktop: `Home icon (16) › Course › Chapter › Note`, `label` 14px, `--text-3` links, current `--text-1` (not a link, `aria-current="page"`). Separators: lucide `chevron-right` 14px `--text-4`. Max widths 220px per crumb with ellipsis. The middle crumbs collapse into `…` (a menu button) if the total overflows.
- Mobile: a single back-crumb, `‹ {parent name}` (lucide `chevron-left` 20 + label, 44px target), truncating at 60vw.
- Animation: crumbs are keyed by route level inside `AnimatePresence`. Entering crumb `x: 8, opacity 0 → 0, 1`, exiting `x: −4, opacity 0` (`dur.fast`). Existing crumbs whose text changes crossfade. `<nav aria-label="Breadcrumb"><ol>`.
- Chapter compact title: on the chapter page after 120px scroll, the last crumb gets `--text-1` weight 580 (it *is* the compact title).

### 6.5 LEDMatrix

The signature component. One component, canvas2D rendering, all sizes.

```ts
type LEDFrame = Uint8Array;               // length 104, row-major (y*13 + x), levels 0..7
type LEDMatrixProps = {
  source?: "stats" | "frame" | "text" | "pattern";
  frame?: LEDFrame;                       // source="frame"
  text?: string;                          // source="text": scrolls once, then onDone
  loopText?: boolean;
  pattern?: "waiting" | "starting" | "x" | "blinkX" | "arrow" | "arrowLoop" | "check" | "404" | "monogram";
  monogram?: string;                      // 1–3 chars, rendered static at x=1 (3 chars) / centered
  size?: "micro" | "xs" | "sm" | "md" | "lg" | "hero";
  plate?: boolean;                        // draw bezel/plate chrome (default true for md+)
  glow?: boolean;                         // default: true in dark theme, md+ only
  interactive?: boolean;                  // hover popover with live stats
  label?: string;                         // accessible name override
  className?: string;
};
```

| Size | Dot Ø | Pitch | Matrix px (13×8) | Plate padding | Where |
|---|---|---|---|---|---|
| `micro` | 2 | 3 | 38×23 | 0 (no plate) | Wordmark in header |
| `xs` | 3 | 5 | 63×38 | 6 | Course card monogram, header "live" pill |
| `sm` | 5 | 8 | 101×61 | 10 | Footer, toasts, empty/error states small |
| `md` | 8 | 12 | 152×92 | 16 | Empty states, 404 on mobile |
| `lg` | 11 | 17 | 215×130 | 20 | Celebration, 404 desktop, course header monogram (use dot 9/pitch 14) |
| `hero` | 14 | 22 | 278×168 | 24 (mobile: scale to fit `min(100%, 340px)`) | Home hero |

**Rendering:**
- Canvas size = matrix px × DPR (cap 2). Pre-render 8 dot sprites per size/theme (core circle in `--led-N`; L4+ adds glow per §2.3, only if `glow`).
- An rAF loop runs only while the view is animated (comet, text, arrow, decay, column slide, heartbeat). It is idle otherwise, so a static monogram costs nothing. Max 30fps for `micro`/`xs`, 60fps for others. Pause when offscreen (IntersectionObserver) or `document.hidden`.
- **Plate chrome (CSS, not canvas):** `--plate` background, radius `xl` (sm: 14, xs: 10), 1px `--plate-bezel` border, `inset 0 1px 0 rgba(255,255,255,0.04)`, `inset 0 -12px 24px rgba(0,0,0,0.35)`, outer `--shadow-2`. `md+` sizes get 4 corner "screws" (4px circles `--plate-screw`, inset 8px) and a silkscreen label under the matrix inside the plate (hero only): `UNO Q · LED 8×13` in `caption`, `--plate-silk`. Hero plate also gets a **glow bloom**: a sibling div with `--glow-lg`, opacity driven by current total brightness (average level / 7 × 0.8 + 0.2, smoothed), so the plate softly "lights the room" when busy.
- The hero plate tilts toward the cursor (max 6deg, `spring.tilt`) on fine pointers.

**Accessibility:** `role="img"`. `aria-label` summarises the state, updated at most every 10s: "Server status: OK. 14 requests per second, 3 photos uploaded in the last 13 seconds." Monograms: `aria-hidden` (the course name is adjacent). If `interactive`, wrap it in a `<button>` that opens the popover (keyboard accessible, `aria-expanded`).

**Idle animations** (when data is flat, i.e. 0 requests for 13s):
- The heartbeat keeps pulsing on every poll (proof of life).
- Every 20–40s (random), with a 1-in-3 chance, the matrix does an idle flourish once, then returns: a single dot "shooting star" diagonal (L7 head, 2-dot tail, 60ms/step), or a scanline sweep across the columns at L3. Never during warning/error/down.

**Monogram rule:** take the initials of the first 3 words of the course name (`"Linear Algebra"` → `LA`, `"Math 101"` → `M1`, single word → first 3 letters: `PHYSICS` → `PHY`). Render with the 3×5 font at y=1..5: 3 chars start at x=1 (occupying 1..11), 2 chars at x=3, 1 char at x=5. Lit at L6 with a single L7 "sparkle" dot. That dot is placed deterministically from a hash of the slug (in the unused rows 0/7), so every course has a unique fingerprint.

### 6.6 Counter

- Anatomy: number (`counter` type, tabular mono) + label (`caption`, `--text-3`).
- **Rolling digits:** split the formatted number (`Intl.NumberFormat`) into characters. Each digit is a `1em`-tall window with `overflow: hidden` containing a vertical 0–9 strip. Animate `y: −digit × 1em` with `spring.snappy` and a 30ms stagger from the rightmost digit. Separators are static.
- **Count-up on first view:** a `useSpring(0, counterSpring)` drives the displayed integer (rounded); the digit strips follow the spring's value. It triggers when 40% in view, once.
- Live updates (stats change): roll only the changed digits, and flash the label to `--accent` for 600ms (`ease.led`).
- `aria-live="off"` for the animation, with the final value in a visually-hidden span.

### 6.7 Dropzone

- Anatomy: container (radius 20, `--surface-1`, dashed-dot border), icon (`image-plus` 28, `--accent`), title (`title-s`), hint (`meta text-3`), buttons row.
- Border: SVG rect overlay with `stroke-dasharray: 0 10`, `stroke-linecap: round`, stroke-width 3, stroke `--border-strong`. This gives a border of dots (on-brand), not dashes.
- **Drag-over:** stroke → `--accent`, dashoffset marches (linear, −20px/600ms loop), background → `--accent-soft`, icon lifts `y: −4` + `spring.pop`, title changes to "Drop to add". Reduced motion: no marching.
- **Invalid drag** (dragging non-image files, detected via `dataTransfer.items[].type`): stroke `--danger`, title "Only photos, please".
- **Error:** stroke `--danger`, shake, message below.
- **Focus:** the whole zone is a button (Enter/Space opens the file picker). Ring.
- **Disabled (10/10 photos):** replaced by `meta` "10 of 10 photos — remove one to add more".

### 6.8 ImageTile (+ blur-up)

Used by NoteCard cover, strip thumbs, lightbox slides.

- Wrapper has an explicit `aspect-ratio: {w}/{h}` (from API) and a background of the **dot shimmer** skeleton.
- `<img loading="lazy" decoding="async" fetchpriority={aboveFold ? "high" : "auto"}>`, starting at `opacity: 0; filter: blur(12px); transform: scale(1.04)`.
- On `load`: animate to `opacity 1, blur 0, scale 1` over `dur.slow` + `ease.outExpo`. Only the first 12 visible tiles animate blur; later ones fade only (perf). Then remove the skeleton.
- **Lightbox blur-up:** layer 1 = thumb (instant, cached) scaled to fit, `filter: blur(6px)` after the morph. Layer 2 = full image, `opacity 0 → 1` after `img.decode()`. When layer 2 is shown, unmount layer 1 after 400ms.
- **Error:** `image-off` icon 20 `--text-3` centered on `--surface-3`, alt text below as `meta`.
- Alt text: `"{note title} — photo {i} of {n}"`.

### 6.9 Lightbox

Specified in §4.5. Component tree: `NoteView (route)` → `Scrim` · `LightboxStage` (`SlideTrack` → `ZoomPane` → `ImageTile`) · `StageControls` (close, counter, prev/next, zoom, download, open) · `ThumbStrip` · `NotePanel` (desktop) | `DetailsSheet` (mobile) · `ShortcutHint`. `role="dialog" aria-modal="true" aria-labelledby={titleId}`. The slide counter has `aria-live="polite"`: "Photo 2 of 4".

### 6.10 Toast

- Position: mobile bottom-center, `bottom: 16px + safe-area + (FAB visible ? 72px : 0)`, width `min(100% − 32px, 420px)`. Desktop bottom-right 24px, width 380.
- Anatomy: `[LED status dot or LED sm icon] [title label 14/560] [body-s text-2] [action ghost button] [× icon-sm]`, radius 14, `--surface-2`, 1px `--border`, `--shadow-3`, padding 12 14.
- Stack max 3. Older toasts scale 0.94 and shift up 8px behind (`layout`, `spring.snappy`). Hover/focus on the stack expands it (desktop).
- Enter: `y: 24, scale: 0.96, opacity 0 → 0, 1, 1` with `spring.ui`. Exit: `x: 40, opacity 0` (desktop) / `y: 24` (mobile), `dur.fast`.
- Swipe to dismiss (`drag="x"` desktop / `drag="y"` down on mobile, threshold 80px or 500px/s).
- Duration: info/success 4s, warning 6s, error 8s (paused on hover/focus/`document.hidden`). **Timer indicator:** a row of 13 micro LED dots along the bottom edge that turn off right→left as time passes (`ease.led`).
- Variants: `success` (check in `--success`), `info` (accent), `warning`, `error`. Container `role="status"` (`role="alert"` for errors).

### 6.11 Header

- Height 56 (mobile) / 64 (≥768). Sticky, glass (§2.8), `--z-header`. The bottom border fades in after 8px scroll. It hides/shows on scroll (§3.9).
- Left: **wordmark**: `LEDMatrix size="micro"` (live mini bar graph, no plate, 38×23) + "notes" in Bricolage 20px/680, −0.02em. It links Home. On hover, the micro matrix scrolls "HI" once.
- Center/left (desktop): breadcrumbs. Mobile: the back-crumb replaces the wordmark on depth ≥ 1 (the wordmark is kept on Home).
- Right: **Live pill** (≥768): `xs` LEDMatrix without plate + `12/s` mono meta, inside a ghost pill 36px. It opens the stats popover. Then the Upload button (chapter page only, after the hero CTA scrolls out, sm primary). Then the **ThemeToggle**.
- Skip link: the first focusable element is "Skip to content", visible on focus.

### 6.12 Footer

- `--bg-1` band, top border `--border-subtle`, padding 48px top / 32px bottom + safe-area.
- Left: `LEDMatrix size="sm"` plate showing live stats. On first view, it scrolls "NOTES SERVER" once, then shows the dashboard.
- Beside it (mono `meta`): `Served from an Arduino UNO Q on your network` / `Up 3d 4h 12m · 12 courses · 1,204 photos` / status badge (`● All systems normal` | `● Storage almost full — uploads paused` | `● Database not responding` | `● Board unreachable`).
- Right: theme selector (full 3-option segmented control: System · Light · Dark), plus a small line "No accounts. No cloud. Just notes."
- Mobile: stacked, plate full-width max 240.

### 6.13 Skeleton / loaders

- **Dot shimmer (skeleton surface):** base `--surface-2` + the dot grid pattern (`radial-gradient` 1.2px dots, 8px pitch, color `--led-1` dark / `rgba(31,94,255,0.10)` light). A shimmer layer above it uses the same dot pattern at `--led-4` (light: 0.22α), masked by a 30%-wide diagonal linear-gradient band. The band moves via `transform: translateX(−100% → 100%)` on a wide child, 1.6s linear infinite, 200ms stagger by index. This reads as "LEDs scanning". Reduced motion: static.
- **Text skeleton:** rounded bars `--surface-3`, heights match line-heights, widths 92% / 64% / 38%.
- **LED loader (inline):** 13 dots (5px, pitch 9) in a row running the firmware comet in 1D: head L7, tail L5, L3, L1, 70ms/step, ping-pong. Used for paging, "Processing on the board…", and button loading (5-dot variant).
- Show skeletons only after 150ms (avoid flashes on LAN, where most responses arrive in < 100ms). Once shown, keep them at least 300ms.

### 6.14 ThemeToggle

- Header: 44×44 icon button cycling **System → Light → Dark** (icons lucide `monitor`, `sun`, `moon`, 20px). Tooltip `Theme: System (dark)`. Icon change: outgoing icon `rotate −90, scale 0.5, opacity 0`, incoming from `rotate 90, scale 0.5`, `spring.snappy`. Sun rays are not animated separately; keep it clean.
- **Transition:** if `document.startViewTransition` exists and motion isn't reduced, run the theme switch inside a view transition and animate `::view-transition-new(root)` with `clip-path: circle(0 at X Y) → circle(R at X Y)` (X, Y = toggle center, R = distance to farthest corner), 560ms `ease.outExpo`. Fallback: set a `theme-switching` class that applies `transition: background-color 200ms, color 200ms, border-color 200ms` on `*` for one frame pair, then removes it.
- The LED field canvas rebuilds its sprites on theme change.
- Footer offers the explicit 3-way segmented control (`layoutId="theme:pill"`).

### 6.15 Input / Textarea

- Label above (`label`, `--text-2`, 6px gap). Required marker is ` *` in `--accent`, with an `aria-required` attribute. Optional fields show `(optional)` in `--text-3`.
- Field: height 48 (mobile) / 44 (desktop), padding 0 14, radius 10, bg `--surface-1` (in the sheet: `--bg-0` in dark for contrast against `--surface-2`), 1px `--border`, text `body` 16px (prevents iOS zoom), placeholder `--text-3`.
- Hover: border `--border-strong`. **Focus:** border `--accent`, plus a 4px `--accent-soft` halo (animated via opacity of a pseudo layer, `dur.fast`), plus a subtle **LED underline**: a 2px row of dots at the bottom inner edge lights left→right (`scaleX` 0 → 1 of a dotted mask layer, 240ms `ease.out`).
- **Error:** border `--danger`, bg `--danger-soft`, icon `circle-alert` 16 at right, message below (`body-s`, `--danger`), entering `height 0 → auto` + opacity (`layout`). `aria-invalid`, `aria-describedby`. Shake on submit-error only.
- **Disabled:** opacity 0.6, bg `--surface-3`.
- Char counter (title): mono `meta` right-aligned under the field, visible after 150 chars, `--warning` at 190, `--danger` at 200.
- Textarea: min 4 rows, auto-grow to 12 rows (`field-sizing: content` where supported, JS fallback), resize none.

### 6.16 UploadTile

See §4.6. States: `idle`, `hover/focus` (ring + remove button visible; always visible on touch), `lifted` (dragging), `uploading(p)`, `done` (L7 bar + check 14px pop), `error` (danger ring + `!` glass badge + message), `invalid` (pre-upload validation failure, striped danger overlay at 12%).

### 6.17 Segmented control

Container 36px, radius 10, `--surface-2`, 1px `--border-subtle`, padding 3. Options `label` 13px. The active pill (`--surface-3` dark / white light, `--shadow-1`) moves via `layoutId` with `spring.snappy`. Arrow keys move selection (`role="radiogroup"`).

### 6.18 State panels (Empty / Error / NotFound)

Shared layout: centered, max-width 420, `LEDMatrix size="md"` with a view pattern, `title-l`, `body text-2`, button row with 12px gap. Enter: plate `scale 0.94 → 1` `spring.soft`, then text fades in with 60ms stagger.

---

## 7. Choreography details for key moments

### 7.1 Home card → Course page

1. Tap: card `scale 0.985` (press). Tilt resets to 0 over 120ms. Grid siblings fade out (160ms).
2. The card surface (`course:{slug}:surface`) expands to the header panel: position + size with `spring.morph`, radius 20 → 28.
3. The monogram grows from `xs` to `lg`-ish (`layout` on wrapper). The canvas swaps to the higher-res version at t=0, and its dots do a single "scanline" as it lands (t = 380ms).
4. The title moves (`layout="position"`) and crossfades from `title-m` to `display-l` over 220ms.
5. The cover photo (if present) fades out during the morph (it's not in the course header).
6. Chapter rows stagger in from `y: 16` at 80ms + 40ms × i. The rail line draws down (`scaleY 0 → 1`, 600ms `ease.outExpo`) behind them.

### 7.2 Chapter row → Chapter page

Index and title morph (§3.8). Meta crossfades. Masonry cards enter as a diagonal wave. The upload CTA pops in last (`spring.pop`, +300ms).

### 7.3 Chapter page → Note view

See §4.5 opening choreography. On the grid underneath, the source card's cover becomes `visibility: hidden` while the morph element is out (motion handles this with layoutId), and the other cards dim under the scrim.

### 7.4 Note cover morph (technical)

The card shows the cover cropped (the card aspect is clamped to 3:4..4:3), while the lightbox shows it uncropped. To morph without distortion:
- The card cover is an outer `div` (clip, `overflow:hidden`, card aspect) containing an inner `m.div layoutId="note:{id}:cover"` sized to the image's **true aspect ratio**, centered (so it overflows the clip on one axis), with `<img style="width:100%;height:100%">`.
- In the lightbox, the same `layoutId` element is sized to fit the stage (true aspect), with no clip.
- Motion animates the inner element's box between the two. The outer clip in the card disappears with the card (it's under the scrim), so you see the photo "uncrop" as it grows. Set `layoutDependency` to the note id and `transition={spring.morph}`.

### 7.5 Upload sheet → New note (success)

The sheet panel has `layoutId="upload:surface"`. At success t=1000ms, set the cover tile's `layoutId` to `note:{newId}:cover`, then navigate. The lightbox mounts with that same ID, so the tile flies from the filmstrip to the stage. The sheet surface fades out (`dur.base`) as the lightbox scrim takes over (both scrims are the same color, so there's no flash). Do not attempt to morph the sheet into the lightbox chrome.

---

## 8. Microcopy

### 8.1 Hero

- **Eyebrow:** `● LIVE ON THE LAN`
- **Headline (pick A):**
  - **A: "Every note from class, in one place."** (recommended: clear, calm)
  - B: "Snap it. Share it. Study it."
  - C: "The class notebook, lit up."
- **Subline:** "Photos of whiteboards, scans and handwritten pages — shared by students, served from a tiny board on your network. No accounts, no cloud."
- **Buttons:** `Browse courses` (primary, smooth-scrolls to the grid) · `How it works` (ghost, opens a popover: "1. Pick a course and chapter. 2. Tap Upload and snap your notes. 3. Everyone on the network sees them instantly.")
- **Plate caption:** `UNO Q · 8×13 · {n} REQ/S · UP {3D 4H}`

### 8.2 Sections & labels

- Home section: "Courses" · meta "{n} total"
- Stats labels: `COURSES` · `CHAPTERS` · `NOTES` · `PHOTOS`
- Course meta: `{n} CHAPTERS · {n} NOTES · {n} PHOTOS`
- Chapter eyebrow: `CHAPTER {position+1, 2-digit}`
- Chapter meta: "{n} notes · {n} photos". Zero: "No notes yet — be first"
- Note card: "{author} · {2h}", "Anonymous" when no author
- Sort: `Newest` / `Oldest`
- End of list: "That's all {n} notes"
- Load more button: "Load more notes"

### 8.3 Buttons

`Upload notes` · `Upload the first note` · `Take photo` · `Choose photos` · `Post note` / `Post note (3 photos)` · `Uploading 2 of 4 · 63%` · `Processing on the board…` · `✓ Posted` · `Cancel upload` · `Keep editing` · `Discard` · `Try again` · `Try again in 12s` · `Download` · `Open original` · `Share` · `Copy link` · `Back to courses` · `Back to course` · `Back to chapter` · `Go back` · `Retry now`

### 8.4 Empty states

- **No courses:** "No courses yet" / "Courses are created by the admin with the `notes-admin` terminal app. As soon as one exists, it'll appear here — no refresh needed."
- **No chapters:** "No chapters yet" / "The admin adds chapters from `notes-admin`. Check back soon."
- **No notes:** "No notes here yet" / "Be the first — snap a photo of your notes and everyone in {chapter} can see them."

### 8.5 Errors & status

- Can't reach server: "Can't reach the board" / "The server didn't answer. It might be restarting — this page will retry on its own."
- Course 404: "This course isn't here anymore" / "It may have been renamed or removed."
- Chapter 404: "This chapter isn't here anymore"
- Note 404: "This note was removed"
- 404 page: "This page isn't on the board." / "The link may be old, or the note was removed."
- Photo failed to load: "Couldn't load this photo"
- Upload errors: see §4.6 table.
- Status badges: "All systems normal" · "Storage almost full — uploads paused" · "Database not responding" · "Board unreachable"

### 8.6 Toasts

- "Posted to {chapter}" + `Copy link` → then "Link copied"
- "New course: {name}" (live poll found one while empty)
- "That's more than 10 — kept the first {n}."
- "You're offline — we'll reconnect automatically." / "Back online."

### 8.7 Voice rules

Second person, present tense, short sentences, no exclamation marks except in celebration-level moments (none currently). Call the server "the board" everywhere. It's friendly and true. Never blame the user ("Something's not right", not "You did X wrong"). Use "photos" in UI copy and "images" only in technical meta.

---

## 9. Performance budget & techniques

- **JS target < 250 KB gzip** (initial route < 170 KB): React + ReactDOM ≈ 60 KB, react-router ≈ 20 KB, motion with `LazyMotion`/`domMax` + `m` ≈ 30 KB, lucide (tree-shaken, ~30 icons) ≈ 8 KB, app ≈ 40–60 KB. Lazy chunks: `NoteView` (~15 KB), `UploadSheet` + celebration (~18 KB). No other runtime deps (no dnd-kit, no confetti lib, no date lib: use `Intl`).
- **CSS** < 30 KB gzip. **Fonts** ≈ 105 KB (§2.4).
- Animate only `transform`, `opacity`. `filter: blur()` is allowed only on ≤ 12 elements simultaneously and never during scroll-linked effects. `backdrop-filter` only on header, scrim, badges. On low-end phones (`hardwareConcurrency ≤ 4` and `pointer: coarse`), the scrim drops blur and the header uses an opaque 94% bg.
- `will-change: transform` only during active gestures/morphs (set/unset). Don't leave it on.
- Masonry and LED canvases are measured/drawn in rAF. There's no layout read in pointermove handlers except cached rects (refresh on scroll/resize).
- Images: `loading="lazy"` beyond the first viewport, `decoding="async"`, and dimensions always known (zero CLS). Preload the next/prev full image in the lightbox (`new Image().src`).
- Stats poll = 1 tiny request/s per open tab. That's fine on a LAN, and it's paused when hidden. The site's own polling will show on the physical matrix, and that is part of the charm, but it means an idle open tab registers 1 req/s. Consider excluding `/api/stats` from `metrics.record_request()` server-side so the graph shows real traffic (recommended).
- Test device: mid-range Android (e.g. Pixel 6a / Galaxy A54) with Chrome performance trace. Hero + scroll must hold 60fps (30fps LED field is acceptable).

---

## 10. Accessibility checklist

- Contrast pairs as documented (§2.1–2.2). Text on photos only via glass badges (≥ 4.5:1 with white).
- Focus ring: `outline: 2px solid var(--focus); outline-offset: 2px; box-shadow: 0 0 0 6px var(--accent-soft)`. Never removed. Visible on all interactive elements, including cards (the ring hugs the card radius) and tiles.
- Keyboard: every flow (browse → chapter → note → gallery → upload → reorder → submit) is completable without a pointer. Upload shortcuts: `U` on a chapter page opens upload (when not typing).
- Dialogs: focus trap, `Esc` closes, focus returns to the originating card/button, background `inert`.
- Live regions: gallery position, upload progress milestones (25/50/75/100% only), reorder announcements, toasts.
- Touch targets ≥ 44×44. Swipe and pinch always have button alternatives (arrows, zoom buttons).
- `lang="en"`, semantic landmarks (`header`, `nav`, `main`, `footer`), headings in order (one `h1` per page: hero headline / course name / chapter title / note title in dialog as `h2`).
- LED visuals are always paired with text equivalents (caption, aria-label, status badge).
- Respect `prefers-reduced-motion` (§3.10), `prefers-contrast: more` (borders → `--border-strong`, glass → opaque), and `forced-colors` (LED canvas replaced by a text status line).

---

## 11. Iconography

- **Set:** `lucide-react` (ISC), stroke width **1.75**, sizes 16 / 18 / 20 / 24, `currentColor`, `aria-hidden` + adjacent text or `aria-label` on icon buttons.
- **Map:** `house` (home crumb) · `chevron-right` / `chevron-left` · `arrow-up-right` (card affordance) · `upload` (Upload notes; FAB) · `camera` (Take photo) · `image-plus` (Choose photos / dropzone) · `images` (count badge) · `x` · `grip-vertical` (desktop reorder hint) · `trash-2` · `download` · `external-link` · `share` · `zoom-in` / `zoom-out` / `maximize-2` · `sun` / `moon` / `monitor` · `triangle-alert` · `circle-alert` · `hard-drive` (507) · `clock` (429) · `wifi-off` · `image-off` · `check` · `rotate-ccw` (retry) · `align-left` (has body hint) · `user` (author) · `link` (copy link) · `keyboard` (shortcut hint).
- **Custom LED glyphs:** anything "status" or "board"-related is drawn with LED dots (LEDMatrix patterns, 5-dot loader), not lucide. That keeps the two icon languages distinct: lucide for actions, LEDs for system state.
- **Favicon:** 32×32 SVG of a 13×8 matrix with a 3-bar graph (`#4C9AFF` dots on `#060A13` rounded square), plus a 180px PNG apple-touch-icon.

---

## 12. File/module structure suggestion

```
src/
  styles/tokens.css        # :root + [data-theme=dark] raw tokens, @theme inline mapping
  motion/springs.ts        # spring, counterSpring, durations, easings, variants
  motion/variants.ts       # pageItem, gridItem(custom), fadeOnly, etc. with reduced variants
  led/font.ts              # 3×5 font copied from firmware
  led/views.ts             # pure functions: (state, t) → LEDFrame (waiting, starting, dashboard, x, arrow, text, check)
  led/LEDMatrix.tsx        # canvas renderer + plate chrome
  led/LEDField.tsx         # hero canvas
  stats/store.ts           # poller + useStats
  components/…             # Button, Badge, Breadcrumbs, Counter, Dropzone, ImageTile, Toast, Skeleton, ThemeToggle, Input, Segmented
  routes/Home.tsx, Course.tsx, Chapter.tsx, NoteView.tsx (lazy), Upload.tsx (lazy), NotFound.tsx
  upload/xhr.ts            # XHR with progress, abort, error parsing (status + {error} + image-index regex)
  upload/celebrate.ts      # particle canvas
```

`led/views.ts` should be pure and unit-tested against the firmware behaviour (bar heights for n = 0, 1, 2, 3, 4, 64, 1000; X coordinates; arrow frames).

---

## 13. Implementation checklist (ordered by visual impact)

1. **Tokens + fonts + themes**: `tokens.css` with both palettes, `@theme inline`, fontsource imports, no-flash theme script, grain + page dot grid. *(Everything depends on this.)*
2. **LEDMatrix component + `led/views.ts` + stats store**: canvas renderer, sprites, plate chrome, firmware-identical dashboard, heartbeat, X, waiting, arrow, text scroll.
3. **Home hero**: LED Field canvas (clouds + cursor + request ripples), hero plate with boot comet, headline word reveal, scroll parallax.
4. **CourseCard**: LED halftone cover reveal, monogram, tilt + spotlight border, diagonal stagger grid, skeletons.
5. **Route transitions + shared elements**: AnimatePresence popLayout, depth-aware direction, `course:*` morphs, scroll restoration.
6. **Chapter page masonry + NoteCard + ImageTile blur-up**: zero-CLS layout, paging with LED loader, end-of-list flourish.
7. **Note view lightbox**: cover morph (§7.4), swipe, keyboard, strip with layoutId indicator, blur-up to 1600px, swipe-down dismiss, then pinch/double-tap zoom.
8. **Upload sheet**: CTA → sheet morph, dropzone (dotted marching border), previews with pop-in, filmstrip Reorder (long-press on touch), validation, XHR progress with per-tile LED bars.
9. **Success celebration**: arrow → particle burst → morph into new note, grid insertion with highlighter pulse, toast.
10. **Upload error states**: per-status panels/banners, per-image regex mapping, 429 countdown ring, 507 warning plate.
11. **Course page chapter rail**: scroll-linked lit rail, rail dots with `ease.led`, thumb-stack fan on hover.
12. **Header + breadcrumbs + live pill + stats popover**: glass, hide-on-scroll, animated crumbs, micro-matrix wordmark.
13. **Counters**: rolling digits, count-up in view, live increments.
14. **Toasts** with LED timer dots; **ThemeToggle** with view-transition circular reveal.
15. **404 page**, empty states, error panels (all using LED patterns).
16. **Footer** with plate and live uptime.
17. **Reduced-motion pass**: verify every item in §3.10 with the OS setting on.
18. **Perf pass**: bundle analysis (< 250 KB gz), low-end device trace, adaptive LED field fps, blur limits.
19. **A11y pass**: keyboard-only walkthrough of every flow, screen reader (VoiceOver iOS + NVDA), contrast audit in both themes, `forced-colors`.
20. Nice-to-haves: paste-to-upload, `navigator.share`, idle matrix flourishes, `U` shortcut, prefetch on hover.

---

## 14. Suggested small server tweaks (to support this design)

These are optional. The design degrades gracefully without them.

1. `GET /api/stats` as described (already planned).
2. Exclude `/api/stats` (and `/files/*`, optionally) from the request counter so the LED graph reflects real activity, not the website's own polling.
3. `GET /api/chapters/{id}?order=desc` for newest-first paging without tail-offset math.
4. Send a `Retry-After` header on 429 responses (tower_governor can be configured to) so the countdown is accurate.
