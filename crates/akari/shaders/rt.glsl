// Ray tracing declarations shared by every RT stage: scene buffers, materials, payloads.
#ifndef RT_GLSL
#define RT_GLSL

#extension GL_EXT_ray_tracing : require
#extension GL_EXT_buffer_reference2 : require
#extension GL_EXT_scalar_block_layout : require
#extension GL_EXT_shader_explicit_arithmetic_types_int64 : require
#extension GL_EXT_nonuniform_qualifier : require

#include "common.glsl"

// Material flags (src/scene.rs MAT_*).
#define MAT_ALPHA_MASK 1u
#define MAT_DOUBLE_SIDED 2u
#define MAT_FOLIAGE 4u       // thin translucent leaves: diffuse transmission
#define MAT_UNLIT_EMISSIVE 8u // emission seen by the camera only (lamp bulbs that also have an analytic light)
#define MAT_CARD_FOIL 16u
#define MAT_GROUND 32u
#define MAT_ROAD 64u
#define MAT_RECOLOR 128u      // the base texture gives light and shade only: colour = its luminance x baseColor (blossom)          // asphalt road ribbon (road.glsl): uv = across / along metres, colour = (half width, kind, seed)        // land-use mapped terrain (ground.glsl): fields carry the ground data    // trading card: texEmissive holds the foil mask (foil.glsl), not emission
#define MAT_GLASS 256u       // thin clear glass: pass through or reflect; shadow rays pass
#define MAT_LAWN 1024u       // a lawn surface: the lawn's blades stand on it (lawn.glsl)
#define MAT_WATER 512u       // a water surface: refract / reflect, ripples, absorption and caustics below
#define NO_TEX 0xFFFFFFFFu

// Mirrors `MaterialGpu` (scalar layout).
struct Material {
    vec4 baseColor;      // linear rgba factor
    vec3 emissive;       // linear rgb (factor x strength)
    float roughness;
    float metallic;
    float alphaCutoff;
    float normalScale;
    float occlusion;     // occlusion strength
    uint texBase;        // indices into textures[], NO_TEX when absent
    uint texNormal;
    uint texMr;          // glTF metallic-roughness: g = roughness, b = metallic
    uint texOcclusion;   // r = occlusion (often the same image as texMr: ORM)
    uint texEmissive;
    uint flags;
    float transmission;  // foliage: fraction of diffuse light passing through
    float uvScale;       // texture LOD helper: mean texels per world metre (1 if unknown)
};

// Mirrors `MeshInfoGpu`: device addresses of the vertex streams (0 when absent).
struct MeshInfo {
    uint64_t pos;
    uint64_t nrm;
    uint64_t uv;
    uint64_t tan;
    uint64_t col;
    uint64_t idx;
    uint material;
    uint flags;
};

// Analytic lights (src/renderer.rs LightGpu).
struct Light {
    vec4 posRadius;  // xyz position, w emitter radius (soft shadows)
    vec4 color;      // rgb intensity (linear, candela-like), w range (m)
    vec4 spot;       // xyz spot direction, w cos(outer angle); w <= -1 for a point light
};

layout(buffer_reference, scalar, buffer_reference_align = 4) readonly buffer Vec3Ref { vec3 v[]; };
layout(buffer_reference, scalar, buffer_reference_align = 4) readonly buffer Vec4Ref { vec4 v[]; };
layout(buffer_reference, scalar, buffer_reference_align = 4) readonly buffer Vec2Ref { vec2 v[]; };
layout(buffer_reference, scalar, buffer_reference_align = 4) readonly buffer UVec3Ref { uvec3 v[]; };

layout(set = 0, binding = 0) uniform accelerationStructureEXT tlas;
layout(set = 0, binding = 3) uniform GlobalsUBO { Globals g; };
layout(set = 0, binding = 4, scalar) readonly buffer Materials { Material materials[]; };
layout(set = 0, binding = 5, scalar) readonly buffer Meshes { MeshInfo meshes[]; };
layout(set = 0, binding = 6, scalar) readonly buffer Lights { Light lights[]; };
layout(set = 0, binding = 7) uniform sampler2D skyLut;
layout(set = 0, binding = 20) uniform sampler2D textures[];

// Object motion (renderer.rs `set_object_motion`): per TLAS instance, last frame's object-to-world
// rows and the mesh it was drawn with (same triangle layout), so moving and deforming objects get
// true motion vectors. Instances past `motionCount` (impostors) or with `valid` = 0 did not move.
struct InstMotion {
    vec4 r0, r1, r2; // previous object-to-world, row-major 3x4
    uint prevMesh;
    uint valid;
    uint pad0, pad1;
};
layout(set = 0, binding = 16, scalar) readonly buffer Motion { uint motionCount; uint motionPad0, motionPad1, motionPad2; InstMotion motion[]; };

// Where the analytic lights reach (renderer/lightgrid.rs): words 0..4 = cell size (float bits),
// slot mask, where the light lists start; then per hash slot (cell x, cell z, first, count;
// count 0 = empty); then the lists of light indices.
layout(set = 0, binding = 17, scalar) readonly buffer LightGrid { uint lightGrid[]; };

// The lights that can reach world point p: (first index into lightGrid, count).
uvec2 lightCell(vec3 p) {
    float cell = uintBitsToFloat(lightGrid[0]);
    uint mask = lightGrid[1];
    if (mask == 0u) return uvec2(0u);
    ivec2 c = ivec2(floor(p.xz / cell));
    uint h = ((uint(c.x) * 73856093u) ^ (uint(c.y) * 19349663u)) & mask;
    for (int k = 0; k < 64; ++k) {
        uint e = 4u + h * 4u;
        uint n = lightGrid[e + 3u];
        if (n == 0u) break;
        if (int(lightGrid[e]) == c.x && int(lightGrid[e + 1u]) == c.y) return uvec2(lightGrid[2] + lightGrid[e + 2u], n);
        h = (h + 1u) & mask;
    }
    return uvec2(0u);
}

// Main payload: what the closest-hit shader found. `t` < 0 on a miss.
// In: `cone` = the ray cone's spread (rad) and width at the origin, for texture LOD.
struct Hit {
    vec3 albedo;
    float t;
    vec3 n;          // shading normal (world, facing the ray's side)
    float roughness;
    vec3 ng;         // geometric normal (world, facing the ray's side)
    float metallic;
    vec3 emission;
    float ao;        // material occlusion (texture)
    vec2 cone;       // in: spread, width at origin
    uint flags;      // material flags
    float transmission;
    float sunVis;    // how much of the sun the surface itself lets through to the point (the lawn's
                     // blades shading each other: lawn.glsl); the ray generation shader sets 1
    vec4 prev;       // out (hit.rchit, object motion): this point's world position last frame,
                     // w = 1 when set; the ray generation shader clears it before each trace
};

// Leaf colour variation over the land (plan xz, so a crown is one colour top to bottom): whole
// stands drift between yellow-green and blue-green and lighter / darker (~40 m), each tree a
// little apart from its neighbours (~7 m). The same for meshes and impostors (not when baking:
// g.opts.z = 1).
float leafNoise(vec2 p) {
    vec2 i = floor(p), f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    float a = float(pcg(uint(int(i.x) + 40000) * 92837u ^ uint(int(i.y) + 40000) * 689287u) & 0xFFFFu) / 65535.0;
    float b = float(pcg(uint(int(i.x) + 40001) * 92837u ^ uint(int(i.y) + 40000) * 689287u) & 0xFFFFu) / 65535.0;
    float c = float(pcg(uint(int(i.x) + 40000) * 92837u ^ uint(int(i.y) + 40001) * 689287u) & 0xFFFFu) / 65535.0;
    float d = float(pcg(uint(int(i.x) + 40001) * 92837u ^ uint(int(i.y) + 40001) * 689287u) & 0xFFFFu) / 65535.0;
    return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
}
vec3 leafVariation(vec3 wp) {
    if (g.opts.z != 0u) return vec3(1.0);
    vec2 p = wp.xz;
    float hue = leafNoise(p / 40.0) * 0.7 + leafNoise(p / 7.0 + 31.7) * 0.3;
    float lum = leafNoise(p / 55.0 + 7.3) * 0.6 + leafNoise(p / 6.0 + 91.1) * 0.4;
    vec3 tint = mix(vec3(1.12, 1.03, 0.78), vec3(0.86, 0.98, 1.12), hue);
    return tint * mix(0.78, 1.18, lum);
}

#endif
