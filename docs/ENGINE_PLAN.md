# Akari engine and content plan

Decisions (2026-11-21):

- **Rendering:** full hardware ray tracing, built on SmartImposter's approach, with DLSS Ray
  Reconstruction and a TAA fallback. Targets RTX cards.
- **Look:** grounded realistic. Photoreal Megascans environments and realistic characters.
- **Levels:** authored in Blender 5.2 (`D:\Blender\blender.exe`) from Fab assets, and exported by a
  Python script the engine reads.

What stays from the first pass: `crates/sim` (calendar, sun/moon, students, bonds, economy, saves)
and `crates/tcg` (cards, seasons, packs), both unit-tested. What gets replaced: the raster
renderer in `crates/akari` and the code-built box levels in `crates/game/src/world.rs`. The box
levels stay only as a greybox until each Blender level exists.

## Phase 1: RT core, ported from SmartImposter

Port the pieces SmartImposter has already proven, trimmed to what a small town needs:

| From SmartImposter | Into Akari | Notes |
|---|---|---|
| `gpu.rs` | `gpu.rs` | Device with RT, SER and NGX extensions; buffers, images, swapchain |
| `accel.rs` | `accel.rs` | BLAS/TLAS; adds per-frame TLAS rebuild and BLAS refit for moving characters |
| `textures.rs`, `bc7.rs` | `textures.rs` | PNG decode, BC7/BC5/BC4 cache, mips |
| `main.rgen`, `tri.rchit`, `tri.rahit`, `miss`, `shadow.rmiss` | same | Primary rays, sun/moon shadows, alpha-tested foliage |
| `atmosphere.glsl`, `clouds.glsl`, `cloudnoise.rs` | same | Driven by `sim::clock::sky` (already matches its sun/moon model) |
| `dlss.rs` + `native/dlss_shim.cpp`, `taa.comp`, `resolve.comp`, `sharpen.comp` | same | DLSS-RR when the SDK is present, else TAA |
| `walk.rs` | `game/player.rs` | First-person walking with ground following and collisions |
| impostors (`impostor.rint`, bake) | later | Distant mountain forests and roadside trees |

New for the game:

- **glTF 2.0 loading** (metallic-roughness PBR, normal maps, alpha mask, emissive) as the one mesh
  format coming out of Blender.
- **Many small lights:** street lamps, shop fluorescents, vending machines and windows at night, with
  ray-traced shadows from the nearest lights.
- **Indirect light for interiors:** the shop, bedroom and classroom must look right lit only by
  windows and lamps. That needs at least one diffuse GI bounce, denoised by DLSS-RR. This is the
  hardest rendering problem in the plan and gets its own screenshot checks.
- **Moving characters:** skinned glTF meshes, with BLAS refit each frame.

**Done when:** a test scene of Megascans assets from `D:\SmartImposter\assets` renders at morning,
noon, dusk and night, outdoors and in an interior room, with no validation errors, at 1440p
DLSS-RR Quality on the RTX 5080.

## Phase 2: Blender pipeline

- `tools/blender/export_level.py`, run headless: `blender.exe level.blend --background --python ...`
  - Writes `assets/levels/<name>/level.glb` (merged static geometry, instanced props).
  - Writes `level.json` with markers taken from Blender empties and their custom properties:
    `spawn`, `exit` (target level and spawn), `collider` (boxes or convex meshes), `zone` (counter,
    bed, classroom), `npc_spot` (`sim::people::Spot` + facing), and `light` (kind, colour,
    night-only).
- `tools/blender/import_fab.py`: brings a Fab/Megascans download into a shared asset library
  `.blend` at real scale, with materials set up for glTF export.
- Collection-instance assets, so changing one asset updates it in every level.

## Phase 3: Path to School (first real level)

This level is finished to final quality before the others start: road, houses, konbini, rice
paddies, Inari shrine, river bridge, level crossing, mountains. Fab provides what it can. Pieces
Fab is unlikely to have get modelled with Blender generators that are textured with Megascans
surfaces:
- Japanese wooden houses with tiled roofs
- utility poles and wires
- block walls
- a vending machine
- torii gates

A Fab shopping list (like SmartImposter's `docs/QUIXEL_DOWNLOADS.md`) goes in `docs/FAB_ASSETS.md`.

## Phase 4: Home (Tsukumodō) and School

Same pipeline and quality bar, with interiors as the focus.

## Phase 5: Characters

Realistic 12–15-year-olds in Japanese school uniforms, plus Uncle Kenji: sourced from Fab or built
in Blender, rigged, with walk/idle/talk animations. Students are placed by `sim` schedules at the
level's `npc_spot` markers.

## Quality gates

- Every phase ends with a fixed set of screenshots (same camera positions and times of day),
  checked before moving on.
- No Vulkan validation errors, and frame time recorded at each gate.
