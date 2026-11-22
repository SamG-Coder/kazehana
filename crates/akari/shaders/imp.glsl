// Plant impostors (ported from SmartImposter): declarations shared by the bake (bake.rgen), the
// intersection shader (impostor.rint) and its closest-hit shader (impostor.rchit).
//
// Every far plant is one AABB in a cell BLAS (renderer/impostor.rs); its record is at
// gl_InstanceCustomIndexEXT + gl_PrimitiveID. Each plant variant has a baked atlas of N x N
// hemi-octahedral orthographic views: albedo (sqrt albedo, leafiness), normal (octahedral, plant
// space) and depth (+ coverage).
#ifndef IMP_GLSL
#define IMP_GLSL

// Mirrors impostor.rs ImpInfoGpu (scalar layout).
struct ImpInfo {
    vec4 centerRadius; // bounding sphere in plant space
    vec4 bmin;         // w: 1 = foliage only (grass: no bark)
    vec4 bmax;         // w: texel size of the atlas (m)
    vec4 frame;        // xy: half the width and height of a view (m); z: leaf transmission
    uint grid;         // views per side
    uint res;          // texels per view: across | up << 16
    uint mips;
    uint tex;          // textures[]: albedo; +1 normal; +2 depth
};

// Mirrors impostor.rs PlantGpu: plant -> world as three rows, variant | seed << 16.
struct PlantRec { vec4 r0; vec4 r1; vec4 r2; uint data; };
mat3 plantMatrix(PlantRec p) { return transpose(mat3(p.r0.xyz, p.r1.xyz, p.r2.xyz)); }
vec3 plantOrigin(PlantRec p) { return vec3(p.r0.w, p.r1.w, p.r2.w); }
uint plantVariant(PlantRec p) { return p.data & 0xFFFFu; }
uint plantSeed(PlantRec p) { return p.data >> 16; }

layout(set = 0, binding = 13, scalar) readonly buffer Imps { ImpInfo imps[]; };
layout(set = 0, binding = 14, scalar) readonly buffer Plants { PlantRec plants[]; };
// Per plant: non-zero = drawn as its real mesh this frame (the impostor steps aside).
layout(set = 0, binding = 15, scalar) readonly buffer PlantState { uint plantState[]; };

// Impostor hit attributes, intersection -> closest hit.
struct ImpHit {
    uint albedo; // unorm4x8: sqrt(albedo).rgb, leafiness
    uint normal; // snorm2x16: octahedral normal, plant space
    uint pad;
};

// Hemi-octahedral mapping of the upper hemisphere (Y up) to [0,1]^2.
vec2 hemiOctEncode(vec3 d) {
    d.y = max(d.y, 0.0);
    d /= (abs(d.x) + d.y + abs(d.z));
    return vec2(d.x + d.z, d.x - d.z) * 0.5 + 0.5;
}
vec3 hemiOctDecode(vec2 uv) {
    vec2 o = uv * 2.0 - 1.0;
    vec2 p = vec2(o.x + o.y, o.x - o.y) * 0.5;
    return normalize(vec3(p.x, 1.0 - abs(p.x) - abs(p.y), p.y));
}
// Orthographic basis of a view; f points from the plant towards the viewer.
void frameBasis(vec3 f, out vec3 r, out vec3 u) {
    vec3 up = abs(f.y) > 0.999 ? vec3(0, 0, -1) : vec3(0, 1, 0);
    r = normalize(cross(up, f));
    u = cross(f, r);
}
vec3 frameDir(vec2 cell, float N) { return hemiOctDecode(cell / (N - 1.0)); }

#endif
