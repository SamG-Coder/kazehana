# Asset quality gate

No 3D asset enters the game until it has passed this gate. This applies to every source: Fab, Fable 5.2 generation, Blender-made models, and anything else.

**The rule is enforced in code.** The game loads models only through `assets::Registry`. The registry refuses:
- any asset that is not approved
- any asset whose files changed after it was approved (the files are hashed at approval)

## The three steps

1. **Automated checks:** run `assetcheck check <file.gltf> --id <id> --category <cat>`.
   - Any FAIL blocks the asset.
   - Each WARN must be acknowledged in the review notes.
2. **Look-dev render:** run `assetcheck lookdev <id>`, which renders the asset in Akari:
   - three camera angles at noon
   - one golden-hour shot
   - an albedo view and a normals view
   - on a neutral grey ground next to a 1.6 m reference figure

   The images go to `assets/qc/<id>/`.
3. **Review and approval:** run `assetcheck approve <id> --reviewer <name> --notes "..."`. Approval is only possible when:
   - the latest check report has no FAIL
   - look-dev images exist for the current file hash

   The record goes to `assets/registry.json`. `assetcheck reject <id> --notes "..."` records a rejection.

## Visual review checklist (step 3)

- **Real-world scale:** check against the 1.6 m figure. A Japanese door is about 1.8 m and a vending machine about 1.83 m.
- **Fits a rural Japanese town:** a grounded, realistic look with no stylised or cartoon mismatch.
- **Materials read correctly** in both lighting setups:
  - no black or missing textures
  - no inverted normals: the normals view shows smooth, consistent colours
  - roughness is believable (no plastic-looking stone)
  - no shiny metal where there should be none
- **Albedo is in a physically plausible range:** no pure-white or pure-black surfaces except snow or charcoal.
- **Foliage:** alpha cut-outs are clean, light comes through the leaves, there are no opaque card edges.
- **Silhouette and detail hold up at play distance:** 1–20 m for props.
- **It sits on the ground:** no floating, no sinking.

## Automated checks

Categories: `prop`, `foliage`, `building`, `character`, `surface`.

| Check | FAIL when | WARN when |
|---|---|---|
| Parse | The glTF is invalid, or a referenced buffer or image is missing | |
| Textures | A texture is not PNG, or is below 1024 px | A texture is not a power of two, or is below 2048 px |
| Base colour | No base-colour texture (except `surface` factor-only) | |
| Normal map | Missing on `prop`, `building`, `character` or `level` (except water, glass and emissive `kz_glow*` / `kz_neon*` / `kz_lamp*` materials) | Missing on `foliage` |
| ORM / metallic-roughness | | Missing |
| Vertex attributes | No POSITION or TEXCOORD_0 | No NORMAL (it gets generated), or a normal map with no TANGENT |
| Scale (height or largest dimension, after node transforms) | Outside the category range (below) | Within 10% of a range limit |
| Origin | The bottom is more than 5 cm above y = 0, or more than 15% of the height (at least 3 cm allowed) below it (not `surface`) | |
| Triangles (LOD0) | Over the category budget, or under 12 (a placeholder) | Over 75% of the budget |
| Degenerate triangles | 1% or more | Any |
| Invalid numbers | NaN or infinite positions or UVs | |
| Texel density (px per m; distant `far_*` scenery excluded) | Under 256 | Under 512, or over 8192 (waste) |
| Foliage alpha | No MASK or BLEND material on `foliage` | BLEND (rendered as MASK) |

Category ranges:

| Category | Size range | Triangle budget (LOD0, per variant) |
|---|---|---|
| prop | 0.03–12 m | 60,000 |
| foliage | 0.05–40 m | 80,000 |
| building | 2–80 m | 400,000 |
| character | height 1.2–2.0 m | 100,000 |
| surface | plane, about 2 m | 50,000 |
| level | 10–3000 m overall | 4,000,000 in total |

## Changing an approved asset

Any change to its files, whether re-exported, retextured or rescaled, invalidates the approval because the hash no longer matches. The asset then has to go through all three steps again.
