# kiln logo

Three concept directions, each as a mark + wordmark lockup (`kiln-logo*.svg`)
and a square mark (`kiln-mark*.svg`, 256x256 viewBox, favicon-safe at 32 px).
Every file has a `-dark` twin tuned for dark backgrounds (GitHub dark is
`#0d1117`). All files are hand-written, self-contained SVG: no fonts, no
`<text>`, no rasters, no external references, no scripts. The wordmark is
drawn as outlined geometric letterforms (x-height 100 u, ascender 144 u,
stem 28 u) whose ascender spans exactly the mark's stem band.

**Recommended: concept B.**

## Concept A: the kiln arch (`concept-a/`)

The direction closest to the original inspiration image: a domed charcoal
kiln with a chimney, a steel voussoir arch ring and keystone, and a fire-lit
mouth in which three flame tongues form a K, flanked by small `< >` template
brackets and set on a hearth with glowing ember lines. It says "oven"
literally and rewards a close look, which suits a README banner, but its
detail is illustration-grade: at 32 px the K and brackets dissolve and only
the glowing arched silhouette remains.

## Concept B: the hot bracket K (`concept-b/`, recommended)

The K is split into its two strokes: a cold steel stem and a white-hot
angle bracket. **K = `|` + `<`**: the C++ template bracket, the thing that
drives compile-time computation, is the part being heated, while the stem
stays cool (it even picks up a faint warm reflection on its fire-facing
edge). The bracket burns from white at its vertex through amber to ember red
at its tips, with a soft glow and a few rising sparks, on a charcoal tile
with heat rising from the floor. It is the boldest shape at every size, the
tile carries its own background so the mark reads identically on light and
dark pages, and the wordmark repeats the idea: its `k` is a stem plus a
fire-gradient chevron, and the `i` carries a flame tittle. `kiln-social.svg`
is a 1280x640 repository social card built from it.

## Concept C: the bracket flame (`concept-c/`)

A symbol with no letter: a flame held between template angle brackets,
`<🔥>`, i.e. "code baked in the oven". The flame is a red-orange envelope
with a white-hot core over a faint heat halo; the brackets are charcoal on
light pages and polished steel on dark. It is the simplest and most
icon-like of the three, and very legible small, but "fire + code brackets"
is a common pairing and the mark alone does not say "kiln".

## Palette

| Role | Hex |
| --- | --- |
| White-hot core | `#fffbe9` |
| Flame yellow | `#ffe17a`, `#ffd23f` |
| Amber | `#ffb43a`, `#ffc64a` |
| Orange | `#ff8a1f`, `#ff7a1a` |
| Flame red | `#e2380f`, `#ef4a12` |
| Ember red | `#a5160a`, `#9e1b0b` |
| Tile / charcoal | `#2b2f37` to `#121418` |
| Steel | `#f3f5f8`, `#aab2bd`, `#9aa3af` |
| Wordmark ink (light) | `#1c1e24` |
| Wordmark ink (dark) | `#f4efe6` |

## Usage

- **README header:** `concept-b/kiln-logo.svg` with `kiln-logo-dark.svg`
  swapped in via `<picture>` + `prefers-color-scheme` (see the repository
  `README.md`). Display at 360-480 px wide.
- **Favicon / avatar / small icon:** `concept-b/kiln-mark.svg` (the tile
  works on any background; `-dark` only lightens the rim).
- **Social preview:** render `concept-b/kiln-social.svg` to PNG and upload it
  under *Settings → Social preview* (GitHub does not accept SVG there).
- Keep clear space of at least the tile's corner radius around the mark; do
  not recolour the fire gradients or set the wordmark in a font.

## Rendering PNGs

With `rsvg-convert` (Debian: `apt-get install librsvg2-bin`):

```sh
rsvg-convert -w 1024 -b white   concept-b/kiln-logo.svg      -o kiln-logo.png
rsvg-convert -w 1024 -b '#0d1117' concept-b/kiln-logo-dark.svg -o kiln-logo-dark.png
rsvg-convert -w 32  concept-b/kiln-mark.svg -o favicon-32.png
rsvg-convert concept-b/kiln-social.svg -o kiln-social.png   # 1280x640
```

Omit `-b` for a transparent background.
