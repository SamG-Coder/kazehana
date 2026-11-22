// The lawn (src/lawn.rs; ported from SmartImposter's shaders/lawn.glsl): short grass as a tiling
// parallax layer, marched in the hit shader of the surface it grows on (hit.rchit: the terrain's
// garden lawns, and MAT_LAWN meshes). Included after imp.glsl (its info is an impostor info with
// grid 0: renderer/impostor.rs bake_lawn).
//
// Tile space: x, z are world metres, y is the height above the surface. A view (frame) from
// direction f stores, for the line along f through every point of the plane of the blade tips,
// the height of the first blade on that line (depth atlas r), its coverage (g), colour, normal
// and sky visibility. The frame coordinates of a point P are therefore
//     uv = (P.xz + f.xz / f.y * (H - P.y)) / T        height = P.y / H
// which is linear in P: a ray is a straight line in every frame, as for the plant impostors.
#ifndef LAWN_GLSL
#define LAWN_GLSL

// The lawn's views: n.x azimuths round the compass x n.y slopes, the slope (cot of the elevation)
// geometric from slopes.x (steepest) to slopes.y (flattest). `cell`: (azimuth, slope); the
// direction points from the lawn towards the viewer.
vec3 lawnFrameDir(vec2 cell, vec2 n, vec2 slopes) {
    float az = cell.x / n.x * 6.2831853;
    float s = slopes.x * pow(slopes.y / slopes.x, cell.y / (n.y - 1.0));
    return normalize(vec3(s * cos(az), 1.0, s * sin(az)));
}
// Where direction v lies among the views: x in [0, n.x) (it wraps), y in [0, n.y - 1].
vec2 lawnFrameCoord(vec3 v, vec2 n, vec2 slopes) {
    float az = atan(v.z, v.x) / 6.2831853;
    float s = clamp(length(v.xz) / max(v.y, 1e-4), slopes.x, slopes.y);
    return vec2(fract(az + 1.0) * n.x, log(s / slopes.x) / log(slopes.y / slopes.x) * (n.y - 1.0));
}

#ifndef LAWN_BAKE_ONLY

struct Lawn {
    uint tex;       // textures[]: albedo (sqrt albedo, sky visibility); +1 normal; +2 height, coverage
    float H, T;     // blade height (m), tile edge (m)
    vec2 n;         // views: azimuths, slopes
    vec2 slopes;    // cot(elevation) of the steepest and the flattest views
    float res, gut; // texels per view, of which gutter on each side
    float mips;
    float lod, marchLod;
    float thr;      // this pixel's coverage threshold
    float cover;    // how much of the lawn's coverage is there (its amount here x distance fade)
};

float lawnHash(vec2 p) {
    uvec2 q = uvec2(ivec2(floor(p)) + 65536);
    return float(pcg(q.x ^ pcg(q.y + 0x9e3779b9u))) * (1.0 / 4294967295.0);
}
float lawnNoise(vec2 x) {
    vec2 i = floor(x), f = fract(x);
    vec2 u = f * f * (3.0 - 2.0 * f);
    float a = lawnHash(i), b = lawnHash(i + vec2(1, 0)), c = lawnHash(i + vec2(0, 1)), d = lawnHash(i + vec2(1, 1));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// Where in the tile the frame coordinates uv are. Every copy of the tile is shifted by its own
// random amount (the tile wraps around: a shifted copy is whole), so no clump lines up with its
// copies into rows; the borders between copies are ragged, not straight cuts through the blades.
vec2 lawnTile(vec2 uv) {
    vec2 c = floor(uv + 0.04 * sin(uv.yx * 39.0) + 0.02 * sin(uv.yx * 101.0 + 1.3));
    uint h = pcg((uint(int(c.x)) * 0x9E3779B9u) ^ pcg(uint(int(c.y)) + 0x51u));
    return fract(uv + vec2(float(h & 0xFFFFu), float(h >> 16)) * (1.0 / 65536.0));
}

vec2 lawnAtlasUV(Lawn m, vec2 cell, vec2 uv) {
    vec2 t = (m.gut + lawnTile(uv) * (m.res - 2.0 * m.gut)) / m.res;
    return (cell + t) / m.n;
}

vec2 lawnDepth(Lawn m, vec2 cell, vec2 uv, float lod) {
    return textureLod(textures[nonuniformEXT(m.tex + 2u)], lawnAtlasUV(m, cell, uv), lod).rg;
}

// Is the point with frame coordinates S at or behind the first blade seen from the frame (and
// within `thick` of it: a blade is a thin sheet, not everything below its tip)?
bool lawnSolid(Lawn m, vec2 cell, vec3 S, float thick) {
    vec2 dz = lawnDepth(m, cell, S.xy, m.marchLod);
    return dz.y * m.cover > 0.02 && S.z <= dz.x && S.z >= dz.x - thick;
}

// One of the four views around direction v (towards the viewer), with probability its sharpened
// bilinear weight: the nearest view nearly always, a mix only half way between two.
vec2 lawnPick(Lawn m, vec3 v, float xi) {
    vec2 gp = lawnFrameCoord(v, m.n, m.slopes);
    vec2 base = floor(gp), fr = gp - base;
    vec4 w = vec4((1.0 - fr.x) * (1.0 - fr.y), fr.x * (1.0 - fr.y), (1.0 - fr.x) * fr.y, fr.x * fr.y);
    w = w * w;
    w = w * w;
    w = w * w;
    w /= (w.x + w.y + w.z + w.w);
    int k = xi < w.x ? 0 : (xi < w.x + w.y ? 1 : (xi < w.x + w.y + w.z ? 2 : 3));
    vec2 cell = base + vec2(k & 1, k >> 1);
    return vec2(mod(cell.x, m.n.x), min(cell.y, m.n.y - 1.0));
}

// March P0 + D s, s in [0, s1], through the view `cell`: the first place the ray goes from free
// space into a blade, decided by coverage once per crossing.
bool lawnMarch(inout Lawn m, vec2 cell, vec3 P0, vec3 D, float s1, out float sHit, out vec2 uvHit) {
    vec3 f = lawnFrameDir(cell, m.n, m.slopes);
    vec2 sh = f.xz / f.y;
    vec3 A = vec3((P0.xz + sh * (m.H - P0.y)) / m.T, P0.y / m.H);
    vec3 B = vec3((D.xz - sh * D.y) / m.T, D.y / m.H);
    // How far behind the first blade of its line a point still counts as blade: thick in the
    // steep views (a ray off its view that slips behind a blade has gone through one), and never
    // longer along the ground than a lawn's blade is high, however flat the view.
    float SHELL = clamp(0.045 / (m.H * max(length(sh), 1e-3)), 0.04, 0.4);
    const float MAX = 32.0;
    float texels = length(B.xy) * s1 * (m.res - 2.0 * m.gut) * exp2(-m.marchLod);
    if (texels > 1.5 * MAX) {
        float up = ceil(log2(texels / (1.5 * MAX)));
        m.marchLod += up;
        m.lod = max(m.lod, m.marchLod);
        texels *= exp2(-up);
    }
    int steps = clamp(int(ceil(max(abs(B.z) * s1 / (0.8 * SHELL), texels / 1.5))), 6, int(MAX));
    float dt = s1 / float(steps), tPrev = 0.0;
    bool inside = false;
    for (int i = 0; i <= steps; ++i) {
        float t = dt * float(i);
        bool s = lawnSolid(m, cell, A + B * t, SHELL);
        if (inside || i == 0) {
            inside = s;
            tPrev = t;
            continue;
        }
        if (s) {
            float lo = tPrev, hi = t;
            for (int k = 0; k < 5; ++k) {
                float mid = 0.5 * (lo + hi);
                if (lawnSolid(m, cell, A + B * mid, SHELL)) hi = mid; else lo = mid;
            }
            vec3 Sc = A + B * hi;
            float cov = lawnDepth(m, cell, Sc.xy, m.marchLod).g * m.cover;
            if (cov > m.thr) {
                sHit = hi;
                uvHit = Sc.xy;
                return true;
            }
            inside = true;
        }
        tPrev = t;
    }
    sHit = s1;
    uvHit = (A + B * s1).xy;
    return false;
}

// How much sun reaches the point P of the slab (0..1): the view nearest the sun is a shadow map,
// P is lit if nothing on its line towards the sun is above it. A few samples up the ray; each
// takes away its coverage where the first blade on its line is clearly above it.
float lawnSun(Lawn m, vec3 P, vec3 L, inout uint seed) {
    if (L.y <= 0.02) return 1.0;
    vec2 cell = lawnPick(m, L, rnd(seed));
    vec3 f = lawnFrameDir(cell, m.n, m.slopes);
    vec2 sh = f.xz / f.y;
    float sExit = (m.H - P.y) / L.y;
    float jitter = rnd(seed);
    float lit = 1.0;
    const int N = 4;
    for (int k = 0; k < N; ++k) {
        vec3 Q = P + L * (sExit * (float(k) + 0.25 + 0.5 * jitter) / float(N));
        vec3 S = vec3((Q.xz + sh * (m.H - Q.y)) / m.T, Q.y / m.H);
        vec2 dz = lawnDepth(m, cell, S.xy, m.marchLod);
        lit *= 1.0 - dz.y * m.cover * smoothstep(0.04, 0.22, dz.x - S.z);
    }
    return lit;
}

// The lawn at the surface the ray hit at distance tHit (world point wp, the surface's up normal
// `up`): `amount` how much of it is lawn there, `tall` the part of the baked height its blades
// stand at, `colour` the lawn's colour there (linear). A blade: the hit moves up to it (tHit),
// in the blade's colour, normal and shade. No blade: the surface between them, in their shade.
// Returns whether the lawn was drawn here at all. `sunVis`: how much sun gets through the blades.
bool lawnShade(uint slot, vec3 wp, vec3 up, float amount, float tall, vec3 colour, float even,
               inout float tHit, inout vec3 albedo, inout vec3 n, inout vec3 ng, inout float ao,
               inout float rough, out float sunVis, out bool blade) {
    sunVis = 1.0;
    blade = false;
    ImpInfo info = imps[slot];
    Lawn lw;
    lw.H = info.centerRadius.x;
    float spread = pl.cone.x, width = pl.cone.y + pl.cone.x * tHit;
    float down = -dot(gl_WorldRayDirectionEXT, up); // how fast the ray comes down on the surface
    float fade = info.bmax.x;
    lw.cover = amount * (1.0 - smoothstep(0.6 * fade, fade, tHit));
    if (lw.H <= 0.0 || down <= 1e-3 || up.y < 0.3 || lw.cover < 0.03) return false;
    lw.tex = info.tex;
    lw.T = info.centerRadius.y;
    lw.slopes = info.centerRadius.zw;
    lw.n = info.bmin.xy;
    lw.res = info.bmin.z;
    lw.gut = info.bmin.w;
    lw.mips = float(info.mips);
    // A real lawn is not one square repeated: it is taller here and shorter there (`even`: an
    // artificial turf is not), and in tufts a foot across with their own shade (gone before a
    // pixel is as wide as a tuft); the tiling is bent by a slow, smooth offset.
    float patches = mix(0.55, 1.0, smoothstep(0.2, 0.8, 0.6 * lawnNoise(wp.xz * 0.23 + 11.3) + 0.4 * lawnNoise(wp.xz * 0.9)));
    float tufts = (1.0 - smoothstep(30.0, 110.0, tHit)) * (1.0 - even);
    vec2 tuft = vec2(lawnNoise(wp.xz * 3.3 + 5.0), lawnNoise(wp.xz * 5.9 + 17.0));
    tall *= mix(patches, 1.0, even) * mix(1.0, mix(0.84, 1.0, tuft.x), tufts);
    vec2 bend = (vec2(lawnNoise(wp.xz * 0.31 + 3.7), lawnNoise(wp.xz * 0.31 + 57.1)) - 0.5) * 0.22
              + (vec2(lawnNoise(wp.xz * 0.07 + 23.9), lawnNoise(wp.xz * 0.07 + 91.3)) - 0.5) * 0.8;
    // tile space: world x and z, y the height above the surface in baked blade heights
    vec3 D = vec3(gl_WorldRayDirectionEXT.x, -down / (up.y * tall), gl_WorldRayDirectionEXT.z);
    float s1 = min(1.04 * lw.H / -D.y, tHit - gl_RayTminEXT);
    vec3 P0 = vec3(wp.x + bend.x, 0.0, wp.z + bend.y) - D * s1;
    vec3 sun = normalize(vec3(g.sunDir.x, g.sunDir.y / tall, g.sunDir.z));
    float texel = lw.T / (lw.res - 2.0 * lw.gut);
    lw.lod = clamp(log2(max(width / texel, 1e-6)) - 1.0, 0.0, lw.mips - 1.0);
    lw.marchLod = floor(lw.lod + 0.5);
    uint seed = hash2(gl_LaunchIDEXT.xy) ^ pcg(g.frame.x * 7919u + 0x1a3fu);
    lw.thr = mix(0.03, 0.97, rnd(seed));
    vec2 cell = lawnPick(lw, normalize(-D), rnd(seed));
    float sH;
    vec2 uvH;
    blade = lawnMarch(lw, cell, P0, D, s1, sH, uvH);
    // Down between the blades without meeting one: from above that is the surface. Seen at a low
    // angle it cannot be (the ray has crossed the whole depth of the lawn sideways): then it is
    // the base of the blades, deep in their shade.
    float deep = 1.0;
    if (!blade && rnd(seed) > smoothstep(0.2, 0.6, -normalize(D).y)) {
        blade = true;
        deep = 0.45;
    }
    if (blade) {
        vec2 auv = lawnAtlasUV(lw, cell, uvH);
        vec4 a = textureLod(textures[nonuniformEXT(lw.tex)], auv, lw.lod);
        vec2 nxy = textureLod(textures[nonuniformEXT(lw.tex + 1u)], auv, lw.lod).rg * 2.0 - 1.0;
        // (the atlas stores sqrt(albedo), the blades baked in a lawn's green: they take on the
        // colour asked for here, keeping their own shades)
        // (`even`: a turf is all its own colour; a lawn only leans towards its ground's)
        vec3 tint = mix(vec3(1.0), clamp(colour / vec3(0.050, 0.085, 0.022), 0.2, 4.0), mix(0.6, 1.0, even));
        albedo = a.rgb * a.rgb * tint * mix(0.82, 1.12, mix(lawnNoise(wp.xz * 0.55 + 41.0), 0.5, even)) * mix(1.0, mix(0.88, 1.1, tuft.y), tufts);
        n = octDecode(nxy);
        if (dot(n, up) < 0.0) n = reflect(n, up);
        ng = up;
        ao = a.a * deep;
        rough = 0.6;
        tHit = tHit - (s1 - sH);
        sunVis = lawnSun(lw, P0 + D * sH, sun, seed) * deep;
    } else {
        ao *= mix(1.0, 0.45, lw.cover);
        sunVis = lawnSun(lw, P0 + D * s1, sun, seed);
    }
    return true;
}

#endif
#endif
