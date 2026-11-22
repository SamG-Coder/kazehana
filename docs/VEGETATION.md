# Vegetation

The town's trees and grass are real scanned plants, drawn the way SmartImposter draws its
forests. The plant meshes come from SmartImposter's species packs (`D:/SmartImposter/assets/species`,
`.mtree` + `.json`). They are Quixel Megaplants and Megascans grass.

## Near: real meshes

Code: `crates/town/src/plants.rs`.

- Every `.mtree` mesh is split into a bark mesh and a leaf mesh (Akari has one material per mesh).
- Leaves are two-sided, translucent foliage. Grass cards are alpha-tested.
- A tree is hundreds to thousands of instances of a few meshes.
- Trees within **70 m** of the camera are drawn as their real meshes.
- Megascans grass patches (wild grass, clumps, dandelions, tall grass, reeds) are scattered on
  meadow, levee and garden ground within **45 m**.
  - They thin out over the outer part of that circle, so there is no edge.
  - A patch is only placed where its whole footprint, out to the patch's real reach plus 0.5 m,
    is grass ground. So none sits on a road (the road ribbon runs 0.25 m past its land-use code)
    or in a paddy.
- Beyond the real patches, the same layout continues as **grass impostors** out to 170 m,
  thinning out over the last 20%.
  - Each patch is real or impostor by the same random number, so the hand-over has no gaps or doubles.
  - The grass ring is its own plant group (`Renderer::set_plant_group(1, ..)`), rebuilt as the
    camera moves without touching the trees.

## Far: impostors

Code: `crates/akari/src/renderer/impostor.rs`, `shaders/impostor.rint`, `impostor.rchit`, `bake.rgen`.

**Bake**
- Each tree variant in use is rendered from 12 × 12 hemi-octahedral directions by `bake.rgen`.
- The bake uses the frame's own hit shaders, so the impostor has exactly the plant's materials
  (blossom recolouring included).
- It records albedo, leafiness, normal, depth and coverage.
- Mips are coverage-weighted. The levels are BC-compressed on all cores and cached in `assets/cache/impostors`.

**Placement and drawing**
- Every tree is one box in a 128 m cell BLAS. Its record holds its transform, variant and seed.
- Plants come in groups, each with its own cells, records and state: the town's trees (group 0)
  and the grass ring round the camera (group 1).
- The intersection shader marches the baked depth of one view chosen near the ray direction,
  parallax-mapped. The coverage test runs once per surface crossing.
- Shadow rays are marched from the light's side, layer by layer, so shadows under crowns are dappled.
- A per-plant state buffer hides a tree's impostor while its real mesh is drawn.

## The town's species

| Planned species (town plan) | Drawn as |
|---|---|
| sugi, hinoki | Norway spruce |
| konara, kunugi, kuri, keyaki | English oak |
| shii, kashi, kusu, tsubaki | European beech (darker) |
| momiji | Norway maple |
| yamazakura, someiyoshino | Norway maple, leaves recoloured to blossom (`MAT_RECOLOR`) |
| ume | rowan, blossom |
| bamboo | giant bamboo |
| kaki, matsu, maki (gardens) | common hazel |

These are the closest Megaplants available. True Japanese species, such as real sugi, cherry and
pine, can replace them once suitable assets are found.
