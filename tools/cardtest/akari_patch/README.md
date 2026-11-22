# Akari patch: trading-card foil (MAT_CARD_FOIL)

Makes the card shinies behave like real holo foil in Akari: the rainbow and pattern colours
shift as the card tilts, and glitter flecks flash. Without it, Akari treats the foil areas as
plain glossy/metal surfaces that only reflect light.

The patch is additive and does not change the material layout. With the new flag set, the
material's emissive-texture slot carries the foil mask instead of emission (cards don't emit).
`renderer.rs` and `gltf_load.rs` are untouched. **Applied** to `crates/akari` (with the user's
OK); this folder keeps the reference copy.

## 1. Copy `foil.glsl` to `crates/akari/shaders/foil.glsl`

## 2. `crates/akari/src/scene.rs`: after `MAT_UNLIT_EMISSIVE`

```rust
/// Trading-card foil: `emissive_tex` holds the card's foil mask (tools/cardgen foil.png, loaded as
/// `TexKind::Data`) instead of emission; the hit shader turns it into view-dependent holo foil
/// (shaders/foil.glsl).
pub const MAT_CARD_FOIL: u32 = 16;
```

## 3. `crates/akari/shaders/rt.glsl`: after `#define MAT_UNLIT_EMISSIVE 8u ...`

```glsl
#define MAT_CARD_FOIL 16u    // trading card: texEmissive holds the foil mask (foil.glsl), not emission
```

## 4. `crates/akari/shaders/hit.rchit`

After `#include "rt.glsl"`:

```glsl
#include "foil.glsl"
```

Replace the emissive texture line:

```glsl
    if (mat.texEmissive != NO_TEX) emission *= tex(mat.texEmissive, uv, TEX_LOD(mat.texEmissive)).rgb;
```

with:

```glsl
    bool cardFoil = (mat.flags & MAT_CARD_FOIL) != 0u; // emissive slot holds the foil mask
    if (mat.texEmissive != NO_TEX && !cardFoil) emission *= tex(mat.texEmissive, uv, TEX_LOD(mat.texEmissive)).rgb;
```

Just before `pl.t = gl_HitTEXT;` (after the normal-map block):

```glsl
    // Trading-card foil (foil.glsl): view-dependent holographic colour, glitter facets.
    if (cardFoil && mat.texEmissive != NO_TEX && m.tan != 0ul && m.uv != 0ul) {
        vec4 f = tex(mat.texEmissive, uv, TEX_LOD(mat.texEmissive));
        Vec4Ref FT = Vec4Ref(m.tan);
        vec4 ft4 = FT.v[tri.x] * w.x + FT.v[tri.y] * w.y + FT.v[tri.z] * w.z;
        vec3 ft = o2w * ft4.xyz;
        ft = normalize(ft - n * dot(n, ft));
        float fsg = (FT.v[tri.x].w < 0.0 ? -1.0 : 1.0) * (back ? -1.0 : 1.0);
        vec3 fb = cross(n, ft) * fsg;
        vec3 rgb = base.rgb;
        applyCardFoil(f, ft, fb, n, -gl_WorldRayDirectionEXT, rgb, metal, rough);
        base.rgb = rgb;
    }
```

## 5. Turn it on in the card test

`tools/cardtest` already loads each card's `foil.png` behind a cargo feature:

```
cd tools/cardtest
cargo run --release --offline --features foil
```

Without `--features foil` the test runs on stock Akari (the foil texture is not bound, so it can
never be mistaken for emission).
