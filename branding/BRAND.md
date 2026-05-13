# TASMail — Brand spec

## Identity

TASMail is a webmail UI for any IMAP/SMTP server (BYOK). The mark is the
**`t@s` envelope** — an open envelope with the wordmark `t@s` set inside it,
where the `@` is rendered in the brand teal as the symbolic centerpiece.
The `t` and `s` flank it in the neutral charcoal of the envelope outline.

The mark works in two modes:

- **Primary mark** — square, the envelope plus inner wordmark
- **Wordmark lockup** — primary mark on the left, the word `TASMail` to the right,
  baseline-aligned to the @ glyph

## Palette

| Role | Token | Hex | Notes |
|---|---|---|---|
| Primary blue | `--tm-blue-600` | `#2563eb` | Used for CTAs, primary brand surfaces, the wordmark accent on light bg. Matches `theme_color` in the PWA manifest. |
| Primary blue (dark) | `--tm-blue-700` | `#1d4ed8` | Hover/active state for primary. |
| Accent purple | `--tm-purple-500` | `#7c3aed` | Hero gradient endpoint with `--tm-blue-600`. Reserved for marketing surfaces. |
| Accent teal | `--tm-teal-400` | `#2dd4bf` | The `@` glyph in the mark. Calls attention to the BYOK email-address symbol. |
| Charcoal | `--tm-charcoal-900` | `#0f172a` | Envelope outline + `t`/`s` glyphs on light bg. |
| Charcoal-2 | `--tm-charcoal-700` | `#334155` | Body copy. |
| Surface | `--tm-bg-light` | `#f8fafc` | Light app surface. |
| Surface (dark) | `--tm-bg-dark` | `#0f172a` | Dark app surface (matches charcoal-900). |

Contrast: every primary surface combination passes WCAG AA at large-text size.
The `@` teal on white scores 3.4:1 — pass for icon use, fail for body type, so
**never use the teal for text**.

## Typography

The wordmark uses **Inter / system-ui** at `font-weight: 800`, letter-spacing
`-0.02em`. The `Mail` portion stays in `--tm-charcoal-900`; only the `TAS` is
optionally tinted `--tm-blue-600` in marketing surfaces.

## Geometry

The square primary mark is built on a **24-unit grid**:

- Outer envelope: `4u × 16u` rectangle at `(4, 6)`, stroke 1.4u, square corners
- Top flap: two diagonals from the top-left and top-right corners meeting at the
  midpoint at `(12, 14)`
- Bottom diagonals: from the same corners running down to the bottom-right and
  bottom-left, intersecting around `(12, 14)`
- Inner wordmark: `t@s` set at `font-size: 7.5u`, baseline at `y = 14.5`,
  centered horizontally
- Clear space: minimum **2u** padding on all sides; never crowd with adjacent
  elements

## Use cases

| Asset | File | Size | Purpose |
|---|---|---|---|
| Primary mark | `build/svg/logo-primary.svg` | 24×24u (vector) | Universal source |
| Dark variant | `build/svg/logo-dark.svg` | 24×24u | For dark backgrounds (envelope becomes white) |
| Mono black | `build/svg/logo-mono-black.svg` | 24×24u | Single-color print, fax, low-fi |
| Mono white | `build/svg/logo-mono-white.svg` | 24×24u | Reverse on solid backgrounds |
| Favicon | `build/ico/favicon.ico` | 16/32/48 | Multi-resolution browser tab |
| Apple touch | `build/png/apple-touch-icon.png` | 180×180 | iOS home-screen |
| PWA icon | `build/png/icon-192.png`, `icon-512.png` | 192/512 | Android + Chromium PWA |
| PWA maskable | `build/png/maskable-192.png`, `maskable-512.png` | 192/512 | Adaptive icons (10% safe-area padding) |
| App icons | `build/app-icons/icon-{16,24,32,48,64,96,128,192,256,512,1024}.png` | 11 sizes | Generic app distribution |
| Open Graph card | `build/social/og-card.png` | 1200×630 | Link previews on Facebook/LinkedIn/Slack |
| Twitter card | `build/social/twitter-card.png` | 1200×600 | Twitter `summary_large_image` |
| Email signature | `build/png/email-signature.png` | 200×200 | Outlook/Gmail signature |
| Wordmark light | `build/wordmark/wordmark-light.svg` | 240×56 | Header on light bg |
| Wordmark dark | `build/wordmark/wordmark-dark.svg` | 240×56 | Header on dark bg |
| Print master | `build/png/logo-2048.png` | 2048×2048 | High-res print/document |

## Don'ts

- Don't recolour the `@`. Teal is the only allowed accent for that glyph.
- Don't add a background to the primary mark. It ships transparent so it can sit on any surface.
- Don't replace the `@` with a different separator — it's the BYOK symbol.
- Don't crop tighter than the 2u clear space.
- Don't render the mark below 16px square — the inner wordmark loses legibility.
