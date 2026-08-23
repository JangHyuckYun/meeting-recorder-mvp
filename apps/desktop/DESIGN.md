# Minute — Desktop Design System

Single source of truth for the Tauri desktop client's visual language.
Tokens live in [`src/styles/global.css`](src/styles/global.css); Tailwind
([`tailwind.config.js`](tailwind.config.js)) is a 1:1 mapping over those tokens;
primitives live in `src/components/ui`; audio/transcript visualizations live in
`src/components/canvas`.

If a value is not in this document, it does not belong in a component.

## 1. Principles

1. **Neutral by mandate.** Slate is the base, one blue is the accent. No purple,
   no gradients, no glassmorphism (`backdrop-filter`), no colored glow shadows.
2. **Token or nothing.** Every color, radius, shadow, duration and type size is
   a token reference. A component that needs a new value extends the token layer
   first; it never inlines a literal.
3. **Honest state.** A transcript hypothesis must never look like a committed
   fact, and a meter must never animate signal it does not have. State is
   encoded in color *and* in a second channel (fill weight, dashed outline).
4. **Data is monospaced.** Timecodes, levels, counts and drift figures use the
   mono stack with tabular figures (`data-numeric`), so numbers stop jittering.
5. **Recording never depends on decoration.** Motion is optional and always
   yields to `prefers-reduced-motion`.

## 2. Color

Colors are stored as bare HSL triplets so one token serves both Tailwind
(`hsl(var(--token) / <alpha-value>)`) and canvas painting.

### 2.1 Palette layer (raw)

| Family | Tokens |
| --- | --- |
| Slate (base) | `--slate-50` … `--slate-950` |
| Blue (accent) | `--blue-50`, `--blue-100`, `--blue-200`, `--blue-400`, `--blue-500`, `--blue-600`, `--blue-700` |
| Status | `--red-*`, `--emerald-*`, `--amber-*` |

Components never reference the palette layer directly except for the one
documented mid-tone (`--slate-700`) used for legacy secondary text.

### 2.2 Semantic layer

| Token | Role | Light value |
| --- | --- | --- |
| `--background` | App canvas | slate-50 |
| `--surface` / `--card` / `--popover` | Panels, cards, menus | white |
| `--surface-sunken` | Inset groups, toolbars | slate-100 |
| `--foreground` | Primary text | slate-900 |
| `--muted-foreground` | Secondary text | slate-500 |
| `--subtle-foreground` | Tertiary text, axis labels | slate-400 |
| `--border` / `--input` | Hairlines, field borders | slate-200 |
| `--border-strong` | Hover borders, scrollbar thumb | slate-300 |
| `--ring` | Focus ring | blue-600 |
| `--primary` (+`-foreground`) | Primary action | blue-600 / white |
| `--primary-soft` (+`-foreground`) | Accent chips, selected rows | blue-50 / blue-700 |
| `--secondary`, `--accent` | Quiet fills, hover surfaces | slate-100 |
| `--destructive`, `--success`, `--warning`, `--info` | Status, each with `-foreground`, `-soft`, `-soft-foreground` | red-600 / emerald-600 / amber-600 / blue-600 |

Tailwind exposes all of these as color utilities: `bg-surface-sunken`,
`text-muted-foreground`, `border-border-strong`, `bg-success-soft`, etc.

### 2.3 Domain tokens

**Caption lifecycle** — mirrors the ASR event contract
(`PARTIAL → STABLE → COMMITTED → REVISED`):

| Token | State | Treatment |
| --- | --- | --- |
| `--state-partial` | hypothesis | slate-400, dashed outline, 20% fill |
| `--state-stable` | endpointed | slate-600, 45% fill |
| `--state-committed` | live window closed | slate-900, 80% fill |
| `--state-revised` | offline correction | blue-600, 90% fill |

Available as `text-state-partial`, `bg-state-committed`, `<Badge variant="partial">`,
and as `palette.state[...]` on canvas.

**Speaker lanes** — `--speaker-1 … --speaker-6` plus `--speaker-unknown`.
Six purple-free hues (blue, teal, amber, rose, sky, slate) chosen to stay
distinguishable side by side; unknown speakers get the neutral token rather than
a color that implies a resolved identity. Labels are meeting-local clusters,
never verified people.

**Audio canvas** — `--wave-active`, `--wave-idle`, `--wave-baseline`,
`--wave-grid`, `--wave-cursor`, `--wave-peak`, `--wave-overlap`.

### 2.4 Dark mode

`.dark` on the root element re-points the semantic tokens (slate-950 canvas,
slate-900 surfaces). `darkMode: ["class"]` is configured; no toggle ships yet,
so every new component must still be written against semantic tokens only —
that is what makes the switch a one-line change later.

## 3. Typography

Stack: `--font-sans` = Pretendard → system UI (Korean-first, no webfont
download in an offline desktop app). `--font-mono` for all data.

| Class | Size / leading | Use |
| --- | --- | --- |
| `text-display` | clamp(38–58px), −0.04em | Recording timer only |
| `text-h1` | 26px / 1.2 | Screen title |
| `text-h2` | 20px / 1.3 | Panel title, dialog title |
| `text-h3` | 16px / 1.4 | Card title, section heading |
| `text-body-lg` | 14px / 1.65 | Reading text |
| `text-body` | 13px / 1.6 | Default UI text |
| `text-label` | 12px / 1.4, 700 | Controls, tabs, select values |
| `text-caption` | 11px / 1.45 | Secondary metadata, badges |
| `text-eyebrow` | 10px, 0.14em, 800 | Uppercase section eyebrows |
| `text-timecode` | 12px mono | Inline timecodes |

Any element rendering numbers gets `data-numeric`, which switches it to the
mono stack with `tabular-nums`.

## 4. Spacing and layout

Tailwind's default 4px scale is the spacing system — `gap-2` (8px), `p-4`
(16px), `mt-3` (12px). Two named layout rails exist because they are structural,
not decorative:

| Token / class | Value | Meaning |
| --- | --- | --- |
| `--rail-width` / `w-rail` | 88px | Left navigation rail |
| `--statusbar-height` / `h-statusbar` | 56px | Live session status bar |
| `--dialog-body-max-h` | 60vh | Scroll ceiling for `DialogBody` |

Canvas geometry cannot use utility classes, so
[`src/components/canvas/geometry.ts`](src/components/canvas/geometry.ts) derives
every pixel constant from the same `GRID = 4` base (`WAVE`, `TIMELINE`,
`TRACKS`). Canvas code imports those constants; it never inlines a number.

## 5. Radius, elevation, motion

| Radius | Value | Use |
| --- | --- | --- |
| `rounded-xs` … `rounded-2xl` | 4 / 6 / 8 / 12 / 16 / 20px | controls → chips → fields → cards → panels → dialogs |
| `rounded-full` | pill | badges, dots |

| Shadow | Use |
| --- | --- |
| `shadow-xs` | resting buttons |
| `shadow-sm` | cards |
| `shadow-md` | hover / raised cards, popovers |
| `shadow-lg` | select menus |
| `shadow-overlay` | dialogs |

All elevation is neutral slate alpha. A shadow tinted with the accent color is a
bug, not a style.

Motion: `duration-fast` (120ms), `duration-base` (160ms), `duration-slow`
(240ms) with `ease-out` / `ease-in-out`. Two keyframes exist: `animate-pulse-dot`
(live recording dot) and `animate-sweep-x` (awaiting-signal indicator).
`prefers-reduced-motion: reduce` disables animation globally in `global.css`.

## 6. Primitives (`src/components/ui`)

shadcn/ui-shaped components built on Radix, `class-variance-authority` and the
token layer. Import from `@/components/ui`.

| Component | Variants | Notes |
| --- | --- | --- |
| `Button` | `primary`, `secondary`, `outline`, `ghost`, `destructive`, `link` × `sm`, `md`, `lg`, `icon` | `asChild` renders onto a child element; defaults to `type="button"` |
| `Card` | `elevation`: flat/raised/floating · `tone`: default/sunken/accent/danger · `interactive` | With `CardHeader`, `CardEyebrow`, `CardTitle`, `CardDescription`, `CardContent`, `CardFooter` |
| `Badge` | `neutral`, `outline`, `primary`, `success`, `warning`, `destructive`, `solid`, plus lifecycle `partial`/`stable`/`committed`/`revised` | `dot` adds a status dot, `pulse` animates it (live states only) |
| `Dialog` | Radix dialog | `DialogHeader`/`DialogBody`/`DialogFooter`; flat scrim, **no** backdrop blur |
| `Select` | Radix select | Token-styled trigger, viewport, item indicator |
| `Tabs` | Radix tabs | Sunken list, raised active trigger |

Every primitive accepts `className` and merges through `cn()`
([`src/lib/utils.ts`](src/lib/utils.ts)), so callers extend rather than fork.

## 7. Canvas components (`src/components/canvas`)

Canvas is used where DOM nodes would not survive the update rate: input levels,
caption timelines and speaker lanes. All three share
[`use-canvas.ts`](src/components/canvas/use-canvas.ts), which owns DPR scaling,
`ResizeObserver` sizing and token resolution
([`src/lib/design-tokens.ts`](src/lib/design-tokens.ts)) — a painter receives CSS
pixels and a resolved palette, never a raw color.

| Component | Props | Behavior |
| --- | --- | --- |
| `WavVisualizer` | `levels?`, `active?`, `height?`, `label?` | Right-anchored bar meter of normalized amplitudes. With no samples it paints an explicit "awaiting signal" baseline — it never fabricates a waveform. Continuous rAF only while sweeping. |
| `CaptionTimeline` | `captions?`, `durationMs`, `cursorMs?`, `onSeek?`, `height?`, `emptyLabel?` | Time ruler with auto-selected tick step, caption blocks tinted by speaker and weighted by lifecycle state, overlap stripe, playhead. With `onSeek` it becomes a `role="slider"` with pointer + arrow/Home/End seeking. |
| `SpeakerTracks` | `speakers?`, `durationMs`, `cursorMs?`, `emptyLabel?` | One lane per diarized speaker with a color chip, truncated label gutter and segment blocks; shared playhead; overlap stripe. Height derives from lane count. |

Rules for canvas work:

- Read colors from the palette (`hsl(triplet, alpha)`), never a literal.
- Empty input renders an empty state, not invented data.
- Only paint continuously (`animate: true`) while something is genuinely live,
  and gate it on `usePrefersReducedMotion()`.
- Every canvas carries `role="img"` + `aria-label`, or slider semantics when
  interactive.

## 8. Rules

| Concern | Correct | Rejected |
| --- | --- | --- |
| Color | `bg-primary`, `text-muted-foreground`, `hsl(var(--speaker-2))` | `#3b82f6`, `rgb(...)`, `bg-blue-600` |
| Spacing | `gap-2`, `p-4`, `GRID * 2` | `margin: 13px`, `style={{ padding: 7 }}` |
| Type | `text-body`, `text-eyebrow` | `font-size: 17px` |
| Radius | `rounded-lg` | `border-radius: 6px` |
| Elevation | `shadow-md` | `box-shadow: 0 8px 22px rgba(...)` |
| Emphasis | New token in `global.css`, then use it | One-off override in a component |
| Depth | Solid surface + hairline border + neutral shadow | Gradient, blur, translucency |

## 9. Migration status

- `src/styles/global.css` is imported before `src/App.css` in `App.tsx`. The
  legacy stylesheet no longer declares its own palette: every legacy rule now
  reads design tokens (`hsl(var(--foreground))`, `var(--shadow-md)`, …), so the
  old screens and the new primitives share one palette. Zero color literals
  remain in `App.css`.
- Purple, gradients and `backdrop-filter` were removed from the legacy sheet;
  the brand mark, nav, buttons and chips are blue/slate.
- Rules whose markup moved to design-system components (`.waveform`,
  `.status-pill`, `.live-transcript-placeholder`) were deleted rather than left
  dangling.
- `LiveRecordingScreen` is the migration reference: `Badge`, `Card` and the
  three canvas components are already wired; the remaining screens keep their
  legacy classes until their own tickets land.
- Tailwind preflight is **on**. Legacy rules set their own borders and margins,
  so they were unaffected; keep it that way when migrating.

## 10. Verification

```bash
pnpm exec tsc --noEmit    # types
pnpm exec vite build      # bundle + Tailwind/PostCSS pipeline
```

Before claiming visual work is done: every color a token, every spacing on the
4px scale, every component composed from the primitives above, and no literal
left behind for a future reader to inherit.
