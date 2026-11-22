# Card templates (Mythbeasts, working name)

Code-generated, two-sided trading card templates. `tools/cardgen` renders every front and the
shared back from the real card data (`tcg::CardDb::generate(12)`). Each template is written as
layered PNGs for the 3D engine plus static previews with the foil simulated.

```
cd tools/cardgen
cargo run --release --offline                      # all templates + contact sheets
cargo run --release --offline -- --game "Mythbeasts"          # rename the game on the back
cargo run --release --offline -- --card S01-023 --finish holo # one card -> templates/cards/S01-023_holo/
```

Finishes: `normal | holo | reverse | fullart | gold | cosmos | ice | rainbow | glass | sakura | obsidian`. Set `CARDGEN_ONLY=<substring>` to render
only matching templates while iterating. A full run takes about 8 s.

## Output (`assets/cards/templates/`)

| Path | What |
|---|---|
| `contact_sheet.png` | All 14 fronts (4 rarities + 10 shiny finishes) and the back |
| `contact_attributes.png` | A Common of each attribute, plus Spell and Relic kind styling |
| `templates.json` | Which card each template shows |
| `<template>/base.png` | Print colour, sRGB RGBA 1500×2100. Alpha is the die-cut shape with rounded corners |
| `<template>/foil.png` | Foil mask, RGBA8 data texture (see below) |
| `<template>/height.png` | Emboss height, 16-bit grey |
| `<template>/spec.png` | R metallic ink, G gloss varnish, B printed-relief amount |
| `<template>/normal.png` | Engine normal map from the emboss height (tangent space; Y = +v, i.e. down the image) |
| `<template>/orm.png` | Engine ORM (glTF): R occlusion, G roughness, B metallic. Foil counts as metal |
| `engine_test/*.png` | Akari ray-traced renders from `tools/cardtest` |
| `<template>/preview.png` | Photo-style render with foil, straight-on camera |
| `<template>/preview_tilt.png` | Same card, camera moved right and down (foil templates only) |
| `<template>/tilt_strip.png` | Five camera angles left→right, showing the foil response |

Templates: `front_common`, `front_uncommon`, `front_rare`, `front_mythic`, `front_holo`,
`front_reverse`, `front_fullart`, `front_gold`, `front_cosmos`, `front_ice`, `front_rainbow`,
`front_glass`, `front_sakura`, `front_obsidian`, `back`. The output runs to a few hundred MB,
mostly 16-bit height maps and previews.

## Physical spec

- 63 × 88 mm, 1500 × 2100 px (≈ 605 dpi). Corner radius 70 px (≈ 3 mm).
- Previews include printed relief shading, card-stock fibre and grain, slight ink mottling, AO under
  raised parts, and a hair of white core showing at the cut edge.

## Look: dramatic and glowy

Frames are near-black lacquer tinted by the attribute, so the art and the light carry the card.
A printed **glow layer** in the attribute colour lights the parts that matter: an energy rim just
inside the art window, the engraved pattern lines nearest the art, an underline in the name
lintel, the type-rail edge, medallion rings, gems and the crest. Glow strength rises with rarity
(Common 0.3, Uncommon 0.5, Rare 0.75, Mythic 1.0; shiny finishes at least 0.6), so tiers read at a
glance. The glow is printed ink baked into `base.png` (hot core + tight bloom + a wide halo
taken from the squared source so dense line work doesn't wash out, damped over the text
plinth); it is not an emissive light. The lintel and type rail are angular "blade" plates with
big diagonal cuts, and the art window and pillars are chamfered.

## Front layout

The frame reads as a small shrine gate: a name **lintel** across the top, two attribute
**pillars** flanking the portrait art, a **type rail**, then a wide text **plinth** and a print
**footer**. Rectangles in px are the `design.rs` constants:

| Element | Rect (x0, y0, x1, y1) | Notes |
|---|---|---|
| Border | 48 px all round | Charcoal; black + double gold pinstripe on Mythic; engraved gold on Gold |
| Name lintel | 64,64 – 1436,190 | Chamfered plate, embossed Palatino Bold name; shrinks, then condenses (≥72%) to fit |
| Attribute medallion | centre 1370,127, r 54 | Enamel dome with the attribute sigil, set in a metal bezel |
| Art window | 240,208 – 1260,1568 | **1020 × 1360, exactly 3:4**, recessed with a metal trim |
| Pillars | 64–240 and 1260–1436, y 196–1584 | Attribute enamel engraved with the attribute's traditional pattern |
| Type rail | 64,1590 – 1436,1666 | Kind badge + kind name on the left, attribute sigil + name on the right |
| Text plinth | 80,1678 – 1420,1976 | Paper tint and outline vary by kind |
| **Reserved rules zone** | 110,1694 – 1390,1858 | Empty except a faint attribute watermark. For future abilities/stats |
| Flavour | below the divider at y 1874 | Italic quote centred; season attribution right-aligned |
| Rarity crest | centre 750,1990 | Sits across the plinth edge and footer |
| Footer | baseline y 2030 | `S01-023  023/060  R  DAWN OF DRAGONS` left · illustrator + © line right |

Stats are not drawn because the rules are not designed. When they are, a stat plate fits in the
bottom-right corner of the rules zone, and an optional cost socket can replace the attribute
medallion. Both can be added without moving anything else.

### Attributes

Colours come from `Attribute::color()`. The pillars, lintel tint, type rail, medallion, gems and
watermark all derive from that one colour. Each attribute also gets a sigil and a traditional
Japanese pattern that is engraved into its pillars and becomes its reverse-holo foil pattern:

| Attribute | Sigil | Pillar pattern |
|---|---|---|
| Flame | flame with an inner tongue | *Uroko* (scales of alternating triangles) |
| Tide | droplet with a wave cut | *Seigaiha* (overlapping wave arcs) |
| Gale | tapering spiral + wind streaks | *Tatewaku* (rising wavy lines) |
| Terra | crystal cluster | *Kikkō* (double-line tortoise-shell hexagons) |
| Radiant | twelve-ray sun | *Asanoha* (hemp-leaf stars) |
| Umbral | crescent with two stars | *Shippō* (interlocking circles) |

### Kinds

| Kind | Badge (type rail) | Icon | Text plinth |
|---|---|---|---|
| Monster | round | three claw slashes | warm ivory, rounded corners |
| Spell | diamond | four-point sparkles | cool blue-grey, concave (arched-cut) corners |
| Relic | hexagon | cut gem | sand/bronze, chamfered corners |

## Rarity and finish designs

Each tier can be told apart at thumbnail size by trim metal, crest shape and ornament:

| Template | Trim | Crest | Ornament | Emboss |
|---|---|---|---|---|
| **Common** | matte gunmetal | bone-white **circle** | none | 0.18 |
| **Uncommon** | bright silver | silver **diamond** | small corner brackets | 0.25 |
| **Rare** | gold | gold foil **5-point star** | scroll filigree at art corners, 4 attribute gems | 0.38 |
| **Mythic** | flame gold, black border with double pinstripe | red **8-point star**, holo stamped, with **wings** | larger filigree, gems, sigil roundels in the pillars, lintel pinstripe | 0.55 |
| **Holo** | as base rarity | as base | as base | 0.45 |
| **Reverse holo** | as base rarity | as base | as base | 0.55 |
| **Full-art** | thin silver hairlines | as base | none: art bleeds to the edge | 1.0 |
| **Gold secret** | all gold foil | gold **8-point star with ring**, rarity printed `SR` | everything | 1.0 |
| **Cosmos** | as base rarity | as base | as base | 0.45 |
| **Cracked ice** | as base rarity | as base | as base | 1.0 |
| **Rainbow** | pearl hairlines (full bleed) | as base | none | 1.0 |
| **Stained glass** | as base rarity | as base | lead lines across the art | 1.0 |
| **Sakura** | rose gold | as base | at least Rare-level filigree | 0.8 |
| **Obsidian** | black chrome, black border | as base | at least Rare-level, gems | 0.8 |

Special finishes print their name in the footer after the rarity letter (e.g. `R · COSMOS`).

Shiny finishes, all original designs:

- **Holo (foil art window).** The art is printed over an "Aurora" foil: soft diagonal spectral
  bands plus sparse glitter flecks. The frame stays as normal print.
- **Reverse holo (foil frame).** Everything except the art window is foil. The foil pattern is the
  card's attribute pattern: each uroko triangle, seigaiha arc or kikkō hexagon is a cell with its
  own phase, so the frame shimmers cell by cell. Foil is weaker under the text plinth (30%) and
  name plate (60%) so text stays legible.
- **Full-art.** The illustration fills the whole card. Lintel, rail and plinth are smoked or frosted
  glass panels (the art blurred and tinted behind them), framed with silver hairlines. A fine
  etched sunburst foil covers the whole card and is also raised in the height map, so the card
  feels textured.
- **Gold secret rare.** Border, body and pillars are gilt foil with engraved guilloche and pattern
  work. Lintel and type rail are black lacquer with foil-stamped gold lettering, the plinth is
  cream, and the art is left untouched.

- **Cosmos.** Nebula foil swirls through the art window (violet, teal and magenta, darker than
  Aurora) with dense star glitter and scattered four-point stars that flash as the card turns.
- **Cracked ice.** The whole card, art included, is cut into long diagonal shards. Each shard
  has its own phase, so they flash independently. The cracks are printed frost-white and raised.
- **Rainbow rare.** A full-bleed card with a pastel spectrum overprinted on the art and glass
  panels (not on the plates or text), strong rainbow foil and fine diagonal etching.
- **Stained glass.** The art window becomes leaded glass: raised dark lead lines (metal) and
  panes that are tinted and foiled one by one, each lighting up when it faces the light.
- **Sakura.** Rose-gold trims and cherry-blossom petals drifting across the frame and art,
  printed pale pink, foil-stamped in rose gold and raised. Petals fade over the text and stay
  off the outer border.
- **Obsidian.** Border, body, pillars and plates are black mirror foil with an oil-slick sheen
  only near the highlight. The engraved pattern glows at full strength and the plinth turns dark
  with light text.

Finishes are layered per pixel (the foil layer stores a pattern id per pixel), so combinations
such as a gold frame with a cracked-ice art window need no new format: only a new finish
pass in `design::finish_pass`.

### Proposed finish model and pull rates (proposal; `crates/tcg` not edited)

The data model only has `Rarity` plus `Pull { foil: bool }`. Proposal:

```rust
pub enum Finish { Normal, ReverseHolo, Ice, Holo, Cosmos, Sakura, Glass, FullArt, Rainbow, Obsidian, Gold }
pub struct Pull { pub id: CardId, pub finish: Finish }
pub struct Owned { pub id: CardId, pub counts: [u32; 11] }   // per Finish
```

| Finish | Eligible | Rate (6-card pack) | Per 20-pack box |
|---|---|---|---|
| Reverse holo | any card in the set | replaces a Common in **1 in 3** packs | ~6.7 |
| Holo | Rare and Mythic | the rare slot is holo in **1 in 6** packs | ~3.3 (the current `foil` rate) |
| Full-art | Rare and Mythic | replaces the rare slot in **1 in 24** packs | ~0.8 |
| Cracked ice | any card | replaces a Common in **1 in 12** packs | ~1.7 |
| Cosmos | Rare and Mythic | the rare slot in **1 in 18** packs | ~1.1 |
| Sakura | Uncommon and up | **1 in 30** packs, spring seasons only (seasonal) | ~0.7 |
| Stained glass | Rare and Mythic | the rare slot in **1 in 40** packs | 0.5 |
| Rainbow | Mythic only | the rare slot in **1 in 80** packs | 1 per 4 boxes |
| Obsidian | Mythic only | the rare slot in **1 in 100** packs | 1 per 5 boxes |
| Gold secret | Mythic only | replaces the rare slot in **1 in 120** packs | 1 per 6 boxes |

Pricing multipliers on top of `base(rarity)`: reverse ×1.5, ice ×2, holo ×3, cosmos ×4,
sakura ×5, glass ×6, full-art ×8, rainbow ×15, obsidian ×18, gold ×25. The rare slot holds at
most one special finish and the reverse/ice slot is separate, so about 1 pack in 4 has a "big"
shiny. Gold
cards print as secret numbers beyond the set (e.g. `S01-061/060`) if the data model later gives
them their own `CardId`. For now they reuse the card's number and print `SR`.

## Card back

One shared back. A deep indigo field with faint seigaiha waves, a guilloche rosette and gold
double rules. The seal in the middle holds the six attribute medallions in a ring around a gold
8-point star. Across it runs a crimson swallow-tail ribbon with the logo, `TRADING CARD GAME`,
and a katakana cartouche (ミスビースト) above it. Gold scroll corners and a © line complete it.
The logo is set from the `--game` argument: the font shrinks or condenses to fit and the ribbon
resizes around the text, so renaming the game is a single flag. The katakana line is in
`design::back` and needs updating by hand if the name changes.

## Art

- Files: `assets/cards/art/Sxx/Sxx-NNN.png` (the `CardDef::art` path). Any size; the image is
  cover-cropped to 3:4 (1020×1360 window). For full-art cards the same file is cover-cropped to
  63:88, which trims about 2.5% from each side; a dedicated taller full-art file can come later.
- When the file is missing, the window is filled with a plain attribute-coloured fill and labelled
  `ILLUSTRATION PENDING · S01-023`. Card art will be designed separately.

## Layers in detail

All layers are 1500×2100 and aligned. The engine samples them with the card's UV (0,0 top-left).

### `base.png`: albedo

sRGB colour as printed. It includes the *printed* illusion of bevels (lit from the top left), as on
a real card, plus card-stock texture. Alpha is the die-cut shape. Use it as albedo; do not
light the bevels again from the base colour.

### `foil.png`: foil mask (RGBA8 data, not a picture)

| Channel | Meaning |
|---|---|
| R | foil strength 0..1 (0 = plain print) |
| G | pattern id × 20: 1 Aurora (holo art), 2 Shatter (reverse frame cells), 3 Etch (full-art), 4 Gilt (gold metal), 5 Silver (reserved), 6 Cosmos, 7 Ice, 8 Rainbow, 9 Glass, 10 Sakura, 11 Obsidian |
| B | phase 0..1: baked per-pixel offset of the rainbow (diagonal for Aurora, per cell for Shatter, angle around the focal point for Etch) |
| A | glitter seed: 0 = no fleck, >0 = a fleck whose value is its facet angle |

Load it as UNORM/linear, not sRGB. Use point or bilinear filtering on R/B/A; decode G with
`round(g * 255 / 20)`, using point sampling or the nearest texel.

### `height.png`: emboss

16-bit, 0 = card surface, 1 = maximum raise. Suggested physical scale: 1.0 → 0.08 mm. Use it
as a normal map (`K ≈ 30` in px space, see below) or parallax; it is too small for
displacement. It holds the physical emboss only (raised name, trims, gems, crest, full-art etch),
scaled by the emboss strength per tier in the table above, plus a faint paper tooth.

### `spec.png`

R metallic ink (silver/gold printed ink and foil), G gloss (varnish: the art window is glossier
than the plinth), B how strongly printed relief was baked into the base colour (informational).

## Foil shader (engine notes)

The preview renderer (`tools/cardgen/src/shade.rs`) implements exactly this, so the previews show
what the shader should produce. The effect comes from the **half vector between light and
view** expressed in the card's tangent plane. As the card rotates, or the camera or a light
moves, the bands sweep and the flecks flash. With ray tracing, run it **once per light sample**
(each RT light or area-light sample gives its own L) and add the results. RT reflections of the
environment can be tinted by the same `foil_color` using the reflection direction as L.

```glsl
// inputs: uv, TBN of the card, V (to camera), L (to light), textures as above
vec3  N    = normalize(TBN * normalFromHeight(height, uv, K)); // K ~ 30 texel-scale slope
vec3  H    = normalize(V + L);
vec3  Ht   = transpose(TBN) * (H - N * dot(H, N));             // tangent-plane half vector
float ang  = (Ht.x * 0.8 + Ht.y * 0.6) * 6.0;                  // diffraction coordinate
float lobe = pow(max(dot(N, H), 0.0), 5.0);                    // closeness to mirror angle

vec4  f   = texture(foilTex, uv);   // R strength, G id*40, B phase, A glitter
int   id  = int(round(f.g * 255.0 / 20.0));
vec3  fc  = foilColor(id, f.b, f.a, ang, lobe);

vec3 base = texture(baseTex, uv).rgb;      // linear
vec3 foiled = (id == GILT || id == SILVER || id == SAKURA)
    ? fc * (0.22 + 1.5 * luma(base))       // metallic stamp: foil is the colour
    : (id == OBSIDIAN) ? fc + base * 0.6   // black mirror: print shows, sheen on top
    : base * fc * 1.3 + fc * 0.12 * sqrt(luma(base)); // ink printed over holo foil
color = mix(litPrint, foiled * lightColor, f.r);

vec3 foilColor(int id, float phase, float spark, float ang, float lobe) {
    vec3 silver = vec3(0.55), c;
    if (id == AURORA) c = mix(silver, spectral(fract(phase + ang * 1.1)), 0.30 + 0.40 * lobe) * (0.55 + 1.0 * lobe);
    if (id == SHATTER) { float fl = 0.5 + 0.5 * sin(TAU * (phase * 5.0 + ang * 2.0));
                         c = mix(silver, spectral(fract(phase * 0.15 + ang * 1.1)), 0.30 + 0.35 * lobe)
                             * (0.45 + 0.8 * lobe + 0.35 * fl * fl); }
    if (id == ETCH)    c = mix(silver, spectral(fract(phase + ang * 1.2)), 0.30 + 0.30 * lobe) * (0.5 + 1.0 * lobe);
    if (id == GILT)    { float b = 0.5 + 0.5 * sin(TAU * (ang * 1.4 + phase));
                         c = vec3(1.0, 0.74, 0.36) * (0.30 + 0.55 * b * b + 1.1 * lobe); }
    if (id == COSMOS)  c = mix(vec3(0.35), mix(spectral(fract(0.62 + phase * 0.5 + ang * 0.7)), vec3(0.25, 0.2, 0.55), 0.35),
                               0.55 + 0.3 * lobe) * (0.5 + 1.1 * lobe);
    if (id == ICE)     { float fl = 0.5 + 0.5 * sin(TAU * (phase * 7.0 + ang * 2.4));
                         c = mix(vec3(0.78, 0.9, 1.0), spectral(fract(phase + ang * 0.6)), 0.25)
                             * (0.3 + 0.55 * fl * fl + 0.9 * lobe); }
    if (id == RAINBOW) c = mix(vec3(0.7), spectral(fract(phase + ang * 0.9)), 0.55 + 0.25 * lobe) * (0.6 + 0.9 * lobe);
    if (id == GLASS)   { float sh = 0.5 + 0.5 * sin(TAU * (phase * 4.0 + ang * 1.5));
                         c = mix(spectral(phase), vec3(1.0), 0.6) * (0.45 + 0.6 * sh + 0.8 * lobe); }
    if (id == SAKURA)  { float b = 0.5 + 0.5 * sin(TAU * (ang * 1.2 + phase));
                         c = vec3(1.0, 0.72, 0.74) * (0.45 + 0.45 * b + 1.0 * lobe) + spectral(fract(ang * 0.8 + phase)) * 0.08; }
    if (id == OBSIDIAN) c = vec3(0.05) + mix(vec3(0.6), spectral(fract(phase + ang * 0.8)), 0.5) * 1.6 * pow(lobe, 60.0);
    if (spark > 0.0)   c += pow(max(cos(TAU * (spark * 13.0 + ang * 2.5)), 0.0), 60.0) * 3.0;
    return c;
}
// spectral(h): r = clamp(|6h-3|-1), g = clamp(2-|6h-2|), b = clamp(2-|6h-4|)
```

Notes:

- `ang` is what animates the effect. Tilting the card by a few degrees moves it by about 1, which
  shifts the Aurora hue by a full cycle across the card, as real holo does. The constants 6.0 and
  1.1 set how fast the bands move; raise them for "busier" foil.
- `lobe` concentrates colour and brightness near the mirror direction, so the foil looks silver-grey
  away from the light and rainbow-bright as the card catches it.
- Glitter: each fleck lights only in a narrow angular window (power 60) chosen by its seed, so
  flecks twinkle as the card moves. Use TAA-friendly clamping, or fade flecks with distance or mip
  level to avoid shimmer.
- Non-foil metallic ink (silver/gold trims on Uncommon/Rare/Mythic) uses `spec.R` as the
  metallic value with a fairly low roughness (≈0.25). Paper uses roughness ≈0.45 modulated by
  `spec.G` (gloss varnish).
- The card is thin: give the 3D mesh a 0.3 mm edge with a grey-white core material, and use the
  back template on the reverse face.

## Akari engine test (`tools/cardtest`)

```
cd tools/cardtest
cargo run --release --offline                    # card inspector window
cargo run --release --offline -- --gate          # renders the shot list to assets/cards/templates/engine_test/
cargo run --release --offline -- --selftest      # drives the inspector, saves inspector_selftest.png, quits
cargo run --release --offline --no-default-features  # stock Akari without the foil shader
```

**Inspector controls:** left-drag rotates the card, right-drag moves the desk lamp around it
(sweep the highlight across the foil), the mouse wheel zooms, Left/Right or 1-9 pick a card
(all 14 fronts and the back), F flips the card, Space spins it, L cycles the lighting (desk lamp
in a dim room / sunlight / lamp only), R resets the view, H hides the help and F12 saves a
screenshot. The panel shows the card, the lighting and the active denoiser.

It's a standalone crate (own `[workspace]`, path dependency on `crates/akari`), so it doesn't
touch the root workspace. Each card is a real 63 × 88 × 0.31 mm slab: a rounded-rect front face
(base / `normal.png` / `orm.png`), the shared back on the reverse face, and an off-white core
edge. The 14 fronts and a face-up back lie on a play mat. Akari path traces the scene (3
bounces) and denoises with **DLSS Ray Reconstruction** (TAA only if DLSS is unavailable). The
tool strips Windows' `\\?\` verbatim-path prefix because NGX can't find `assets/dlss/nvngx_dlssd.dll`
through it.

**What works on stock Akari:** emboss (normal map), metallic ink and gilding, gems, varnish
reflections, the gold and obsidian metals. Foil areas fall back to a glossy, partly metallic
surface (ORM metallic 0.35), so they shine but don't shift colour.

**The real holo effect: Akari foil shader (applied).** `crates/akari/shaders/foil.glsl` plus
small additive edits to `scene.rs`, `rt.glsl` and `hit.rchit` (a `MAT_CARD_FOIL = 16` flag;
the foil mask rides in the material's emissive-texture slot, so the material layout,
`renderer.rs` and `gltf_load.rs` are unchanged and other materials behave as before). In the
hit shader the foil colour follows the view direction in the card's tangent plane, so the
rainbow sweeps as the card tilts, while the path tracer supplies the real highlights. Glitter
flecks tilt the shading normal a little (scaled by foil strength) so they flash at the lamp.
The card test's `foil` feature is on by default; it uses `MAT_CARD_FOIL`, so it won't compile
against an Akari without the shader, and the foil texture can never be mistaken for emission.
Exact edits:
`tools/cardtest/akari_patch/README.md`.

Known to tune: on the gold card at grazing angles the gilt border picks up a slightly blotchy
teal cast (likely BC7 compression of the pattern-id channel or mip blending of ids at foil
edges); a point-sampled or BC-free foil mask would fix it.

## Booster packs (`tools/cardgen --packs`, `assets/cards/packs/`)

### Research: what real packs look like

| Format | Cards | Wrapper (W x L x D) | Packs per box | Source |
|---|---|---|---|---|
| Japanese expansion pack | 5 | 66 x 135 x 2.5 mm | 30 | measured packs; Japanese retailers list 5 cards x 30 packs |
| Japanese high-class / premium | 10-11 | 70 x 135 x 3.5 mm | 10 | measured packs; high-class boxes are 10 packs |
| Western retail booster (modern) | 10 (+ code card) | 66 x 120 x 3-4 mm | 36 | measured packs; 36-pack display boxes |
| Western booster (2000s) | 11 | 66 x 115 x 4 mm | 36 | measured packs |
| **Mythbeasts (crates/tcg)** | **6** | **66 x 128 x 3 mm** (ours) | **20** | `PACK_SIZE = 6`, `PACKS_PER_BOX = 20` |

The idea that a Mythbeasts pack holds 20 cards comes from `PACKS_PER_BOX = 20`: the game's rules
put **6 cards in a pack and 20 packs in a box**. Other formats are kept as options because the
wrapper is generic. (Pack measurements come from a collectors' forum and are accurate to about
1 mm. No maker publishes wrapper sizes.)

How a pack is made: one sheet of **metallised BOPP film** is printed on its outside and folded
around the cards (flow-wrap).
- A **fin seal** runs down the middle of the back, formed by pressing the two film edges together and folding the flap flat.
- **Serrated crimp seals**, about 10-12 mm deep, close the top and bottom.
- A **V-notch** is cut into the side folds just under the top crimp. Tearing from it across the pack takes off the top strip.
- The inside of the film is bare silver. A torn edge shows a whitish, stressed film core.

### Formats (`tools/cardgen/src/pack.rs` `FORMATS`)

| Slug | Cards | W x L x D (mm) | Crimp | Tear line | Box |
|---|---|---|---|---|---|
| `booster6` | 6 | 66 x 128 x 3.0 | 11 | 12.8 mm from the top | 20 |
| `jp5` | 5 | 66 x 135 x 2.5 | 12 | 13.8 | 30 |
| `retail10` | 10 | 66 x 120 x 4.0 | 10 | 11.8 | 36 |
| `premium11` | 11 | 70 x 135 x 3.5 | 12 | 13.8 | 10 |

To add a size, add a row to `FORMATS`. The net, zones, guides and the 3D pack all follow from it.

### Print net and layers (`assets/cards/packs/<slug>/`)

The net is the flat film at 16 px/mm (406 dpi). From left to right:
`fin A | back, right half | FRONT | back, left half | fin E`, with the crimps across the top and
bottom (the `booster6` net is 2368 x 2048 px).

| File | What |
|---|---|
| `base.png` | Final albedo (sRGB) of the whole net |
| `metal.png` | 1 where the bare metallised film or a metallic ink shows, 0 under opaque ink |
| `height.png` | Relief (16-bit, mid grey = 0, +/-0.1 mm): crimp V-ribs at 1 mm pitch, diamond knurl on the fin, a faint film orange-peel |
| `normal.png`, `orm.png` | Engine maps (tangent space, Y = +v; ORM R=1, G roughness, B metallic) |
| `guides.png` | Dieline overlay (RGBA). Magenta: cut. Cyan, dashed: side folds. Yellow: fin seal, plus the fin keep-out on the back. Red hatch: crimps. Green: tear line and notches. Blue, dashed: 3 mm safe area. White: design zones |
| `template.png` | `base` with the guides on top: the reference sheet for artists |
| `preview_front.png`, `preview_back.png` | Flat panel previews |
| `pack.json` | Every dimension in mm, plus the zone rectangles (front: logo, art, footer, badge; back: info, barcode, icons) |

`assets/cards/packs/contact_packs.png` shows every format, front and back, at true relative size.

The generic design is deliberately plain so graphics can drop in on top:
- a midnight ink ground;
- a metal sunburst behind the logo;
- the game name in metallic gold (from `--game`);
- a flat colour placeholder in the art window;
- a card-count badge, a "BOOSTER PACK" footer and TEAR markers;
- on the back, a copy-block placeholder, a barcode box and an icons row.

**Graphics layers** (optional files in `assets/cards/packs/art/`; each covers what is beneath it, using its alpha):

| File | Covers |
|---|---|
| `<slug>_art.png` | Inside the art window only. The frame, badge and logo stay on top |
| `<slug>_front.png`, `<slug>_back.png` | The whole front or back panel (W x L, any resolution, stretched). The back is drawn as seen from behind |
| `<slug>.png` | The whole net, as a full print file |
| `<slug>_metal.png` | Greyscale over the net: white shows bare film (metal) through the ink |

Re-run `cargo run --release -- --packs` in `tools/cardgen` (`CARDGEN_ONLY=jp5` renders one format).

### The 3D pack and the tear (`tools/cardtest`, `src/pack.rs`)

`cardtest --pack [--format jp5]`, or press P in the card inspector.

**Geometry.** The wrapper is built from `pack.json`:
- It is one closed tube of film. Columns are packed closer near the folds, and the cross-section is a superellipse, so the folds come out round.
- It pinches flat into the crimps. The two film layers sit 0.03 mm apart there, so they never overlap.
- It puffs to the full thickness D where the cards are, and less in the headroom above them.
- Soft wrinkles radiate where the film is pinched toward the crimps.
- The inside is a second surface one film thickness in, in silver.
- A thin white cut face joins the outside and inside along the tear.
- The folded fin lies on the back.

**Tear line.** The tear line is ragged: three octaves of noise, about ±0.4 mm. It splits the tube into the body and the top strip, and both pieces share exactly the same edge.

**Peeling.** The tear runs from the notch you grab:
- Points behind the tear front bend up around it on a 3 mm radius, to about 126°.
- They then continue straight, lifting as the hand pulls, with the top edge leading slightly, as a held strip twists.
- The strip's shape depends only on tear progress. It is pre-built as 72 meshes per direction, which puts consecutive tear fronts about 1 mm apart.
- The body has 10 mesh states for the mouth opening, with a lens-shaped mouth wide enough for the stack.
- Nothing is rebuilt on the GPU while you tear.

**Feel.**
- The tear front tracks the cursor, which is projected onto the pack plane through the real camera ray, with a critically damped follow. It is smooth at any frame rate and exact once the hand stops.
- The tear only ever moves forward. Let go part way and the flap stays; grab it again to finish.
- The pack rolls slightly against the pull and springs back.

**Opening and reveal.**
- When the tear completes, the strip flies off, the mouth opens and the cards slide up.
- Click or press Space to draw. The stack comes out and turns face-up toward you.
- Each click deals the top card onto a pile. The rare comes last.
- The last click fans all the cards out, with the rare forward.
- The cards are rolled with `tcg::open_pack`: 4 C, 1 U, 1 R/M, and a foil in 1 pack in 6, which shows as the holo template.
- Other formats add or drop commons. That is a display stand-in; only crates/tcg defines the 6-card pack.

**Keys.**
- Drag from a notch to tear; left-drag anywhere else to rotate; right-drag to move the lamp.
- Click or Space draws the next card.
- Tab changes the format, N opens a new pack, P switches between cards and packs.

**Scripted test.** `cardtest --pack-test [--format slug]` runs a scripted opening in a hidden window on a fixed 60 Hz clock. It saves stills to `assets/cards/packs/engine_test/<slug>_1_sealed … 9_fan_tilt.png`, including close-ups at the start, middle and end of the tear.

**Object motion vectors (Akari).** The inspector calls `Renderer::set_object_motion(true)`, so things that move on their own (the peeling strip, the flying strip, the dealt cards) get true motion vectors and leave no DLSS/TAA ghost trail.

How it works:
- Each frame the renderer matches instance `i` with instance `i` of the frame before.
- When the transform changed, or the mesh is a different one with the same vertex and index counts (a pre-built deformation state such as the tear meshes), it writes the previous object-to-world transform and previous mesh to a per-instance buffer (`rt.glsl` `Motion`, binding 16).
- `hit.rchit` re-evaluates the same triangle and barycentrics on that mesh and transform, and returns last frame's position in the payload (`Hit.prev`).
- `pt.rgen` projects that position, instead of the static one, into the motion vector.

It is off by default. Turn it on only where the instance list keeps a stable order from frame to frame.

## Fonts (Windows system fonts for now)

| Use | Font | File |
|---|---|---|
| Card name, logo | Palatino Linotype Bold | `C:/Windows/Fonts/palab.ttf` |
| Flavour text | Palatino Linotype Italic | `palai.ttf` |
| Type rail, labels, set code | Franklin Gothic Medium | `framd.ttf` |
| Fine print | Bahnschrift (default instance) | `bahnschrift.ttf` |
| Katakana on the back | Yu Gothic Bold | `YuGothB.ttc` |

These are not redistributable with a shipped game. Before release, swap them for licensed or
OFL fonts (e.g. a book serif such as EB Garamond or Cormorant for names, and Barlow
Semi-Condensed or Oswald for labels). `text::Fonts::load` is the only place to change.

## Code map (`tools/cardgen/src`)

| File | Role |
|---|---|
| `main.rs` | CLI, showcase selection, layer export, contact sheets, tilt strip |
| `design.rs` | Front/back layout, rarity styles, finishes, foil passes |
| `canvas.rs` | Layered pixel canvas (colour, height, relief, env, foil, metal, gloss, region) and PNG I/O |
| `motifs.rs` | Sigils, kind icons, crests, scroll filigree, the six patterns, guilloche |
| `shade.rs` | Bakes printed relief/AO/paper, writes layers, preview renderer + `foil_color` |
| `art.rs` | Art loading (cover crop) and the placeholder fill |
| `shade.rs` `engine_maps` | Engine normal + ORM export |
| `text.rs` | fontdue text setting: kerning, tracking, fit-to-width, wrap |
| `sdf.rs`, `math.rs` | Signed distance shapes, colour space, noise |
| `pack.rs` | Booster pack formats, print net, generic design, graphics layers, guides, `pack.json` |

The crate has its own empty `[workspace]` so it is not part of the root workspace. It builds
offline from the local cargo cache (fontdue 0.9, png 0.17, rayon).
