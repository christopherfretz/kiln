# kiln logo

**The kiln logo is B3** (concept B with a gear frame and a C++ plate). Its
canonical files live here at the top level:

| File | Use |
| --- | --- |
| `kiln-logo.svg` / `kiln-logo-dark.svg` | mark + wordmark lockup |
| `kiln-banner.svg` / `kiln-banner-dark.svg` | lockup + tagline "C++26 LLVM FRONTEND IN RUST" (README header) |
| `kiln-mark.svg` / `kiln-mark-dark.svg` | square mark with the C++ plate (avatars, 64 px and up) |
| `kiln-favicon.svg` / `kiln-favicon-dark.svg` | square mark without the plate (favicon, 16-48 px) |
| `kiln-social.svg` | 1280x640 repository social card (dark) |

The mark: a 16-tooth steel gear ring (a nod to Rust, deliberately not its
logo: no R, no rivets, different tooth count and shape) frames a charcoal
hub; inside, a K made of a cold steel stem and a white-hot C++ template
bracket (**K = `|` + `<`**); a charcoal plate reading `C++` is bolted
across the lower rim. With the plate present the K is optically centred
between the hub's top and the plate rather than in the full circle; the
plate-free favicon centres it in the circle.

All files are hand-written, self-contained SVG: no fonts, no `<text>`, no
rasters, no external references, no scripts. The wordmark is outlined
geometric letterforms (x-height 100 u, ascender 144 u, stem 28 u) whose
ascender spans the mark's stem band; the tagline is stroked monoline caps.
Each wordmark glyph is a single outline (the n's stem and arch, and concept
A's k, are unioned into one path), so no anti-aliasing seam can appear where
sub-shapes would otherwise meet.
Each `-dark` file is tuned for dark backgrounds (GitHub dark is `#0d1117`).

The exploration that led here is kept below: three first-round concepts
(`concept-a/`, `concept-b/`, `concept-c/`) and the concept B variants
(`concept-b/variants/b1`, `b2`, `b4`; B3 is the top-level set).

## Concept A: the kiln arch (`concept-a/`)

The direction closest to the original inspiration image: a domed charcoal
kiln with a chimney, a steel voussoir arch ring and keystone, and a fire-lit
mouth in which three flame tongues form a K, flanked by small `< >` template
brackets and set on a hearth with glowing ember lines. It says "oven"
literally and rewards a close look, which suits a README banner, but its
detail is illustration-grade: at 32 px the K and brackets dissolve and only
the glowing arched silhouette remains.

## Concept B: the hot bracket K (`concept-b/`, chosen)

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

## Concept B variants: C++ and Rust cues (`concept-b/variants/`)

Four refinements of concept B that say "C++" and "Rust" explicitly while
keeping its look. The gear is a nod to Rust, not its logo: 16 chunky
trapezoid teeth, a plain bore, no R and no rivets.

- **B1, gear frame:** the tile becomes a 16-tooth steel gear ring; the hot K
  sits in its charcoal hub. Survives at 32 px as a toothed ring.
- **B2, C++ plate:** the original tile with a chamfered charcoal base plate
  reading `C++` in glowing amber (drawn as stroked paths); the wordmark's `i`
  carries a small hot gear instead of a flame.
- **B3, gear frame + C++ plate (chosen; files at the top level):** B1's
  gear with B2's plate bolted across its lower rim, echoing the original inspiration
  image's kiln-with-gear-and-badge while staying geometric.
- **B4, K++:** the original tile with two hot `+` marks set in the
  bracket's mouth, so the mark reads `K++`; gear tittle in the wordmark.

Each variant has `kiln-mark`, `kiln-logo` and `kiln-banner` (lockup with
the tagline "C++26 LLVM FRONTEND IN RUST" in stroked monoline caps),
each with a `-dark` twin, plus `kiln-social.svg` (1280x640); B3's set is
the top-level files. B2 and B3 add `kiln-favicon[-dark].svg`: the plate is
unreadable at 32 px, so the favicon drops it (B3's favicon is the B1 gear
mark). In the final B3 the K was lowered from the circle's centre so it sits
optically centred between the hub top and the plate.

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

- **README header:** `kiln-banner.svg`, with `kiln-banner-dark.svg` swapped
  in via `<picture>` + `prefers-color-scheme` (see the repository
  `README.md`). Display at about 480 px wide. Use `kiln-logo*.svg` where the
  tagline would repeat nearby text.
- **Favicon / small icon (under 64 px):** `kiln-favicon.svg`; the C++ plate
  is unreadable that small.
- **Avatar / app icon (64 px and up):** `kiln-mark.svg`.
- **Social preview:** render `kiln-social.svg` to PNG and upload it under
  *Settings → Social preview* (GitHub does not accept SVG there).
- Keep clear space of at least one gear tooth around the mark; do not
  recolour the fire gradients or set the wordmark in a font.

## Rendering PNGs

With `rsvg-convert` (Debian: `apt-get install librsvg2-bin`):

```sh
rsvg-convert -w 1024 -b white     kiln-banner.svg      -o kiln-banner.png
rsvg-convert -w 1024 -b '#0d1117' kiln-banner-dark.svg -o kiln-banner-dark.png
rsvg-convert -w 32 kiln-favicon.svg -o favicon-32.png
rsvg-convert kiln-social.svg -o kiln-social.png   # 1280x640
```

Omit `-b` for a transparent background.
