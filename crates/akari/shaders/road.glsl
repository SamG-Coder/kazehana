// Asphalt roads (MAT_ROAD): the asphalt scan, with scanned Megascans decals (cracks, repair
// patches, potholes) scattered along it, plus the large-scale wear of a used country road:
// resurfaced stretches of different age, polished wheel paths, the occasional long utility-cut
// patch, bites out of the edge with dust and grit, manhole covers, worn paint, damp ruts.
//
// Mesh contract (crates/town): uv = (metres across from the centre line, left positive;
// metres along the road), colour stream = (half width, kind, seed). Kinds: 0 street / lane,
// 1 prefectural road (dashed centre line), 2 old road, 3 school road.
// The material's textures are the asphalt scan (B, N, ORM consecutive from texBase); the decals
// follow from texEmissive, 3 textures each (colour + opacity, normal, AO / roughness / height),
// cracks first, then patches, then potholes. emissive = (cracks, patches, potholes) counts,
// baseColor.xyz = their sizes in metres (square).
#ifndef ROAD_GLSL
#define ROAD_GLSL

#include "ground.glsl"

float r_hash1(float x) { return fract(sin(x * 127.1 + 311.7) * 43758.5453); }
float r_hash2(vec2 p) { return g_hash(p); }
float r_noise1(float x) {
    float i = floor(x), f = fract(x);
    return mix(r_hash1(i), r_hash1(i + 1.0), f * f * (3.0 - 2.0 * f));
}

// Distance to the nearest cell edge of a jittered grid (Voronoi F2 - F1): crack networks.
float r_cellEdge(vec2 p) {
    vec2 i = floor(p), f = fract(p);
    float d1 = 8.0, d2 = 8.0;
    vec2 m1 = vec2(0.0);
    for (int y = -1; y <= 1; ++y) {
        for (int x = -1; x <= 1; ++x) {
            vec2 g = vec2(x, y);
            vec2 o = vec2(g_hash(i + g), g_hash(i + g + 17.3));
            vec2 r = g + o - f;
            float d = dot(r, r);
            if (d < d1) { d2 = d1; d1 = d; m1 = r; } else if (d < d2) d2 = d;
        }
    }
    return sqrt(d2) - sqrt(d1);
}

// A crack line of width w (m) at distance d: coverage, faded to its average where the pixel
// footprint is wider than the crack.
float r_crack(float d, float w, float footprint) {
    float sharp = 1.0 - smoothstep(w * 0.5, w * 0.5 + max(footprint * 0.5, 0.003), d);
    float avg = clamp(w / max(footprint, 1e-4), 0.0, 1.0) * step(d, footprint);
    return footprint > w * 2.0 ? avg * 0.6 : sharp;
}

// One decal (index k) centred at c in road coordinates, turned by ang, `size` metres square:
// blends its colour, normal, roughness and occlusion in by its opacity.
void r_decal(uint first, int k, vec2 rc, vec2 dDx, vec2 dDy, vec2 c, float ang, float size, float tint,
             inout vec3 alb, inout vec2 nOfs, inout float rough, inout float ao) {
    float cs = cos(ang), sn = sin(ang);
    mat2 R = mat2(cs, -sn, sn, cs); // road -> decal
    vec2 uv = R * (rc - c) / size + 0.5;
    if (any(lessThan(uv, vec2(0.0))) || any(greaterThan(uv, vec2(1.0)))) return;
    uint ic = first + uint(k) * 3u;
    vec2 gx = R * dDx / size, gy = R * dDy / size;
    vec4 col = textureGrad(textures[nonuniformEXT(ic)], uv, gx, gy);
    float o = col.a;
    if (o < 0.004) return;
    vec2 nt = textureGrad(textures[nonuniformEXT(ic + 1u)], uv, gx, gy).rg * 2.0 - 1.0;
    vec3 d = textureGrad(textures[nonuniformEXT(ic + 2u)], uv, gx, gy).rgb;
    // the decal's normal is in its own frame: back into the road's (ground x / z) frame
    nt = transpose(R) * nt;
    vec3 dc = col.rgb;
    float dl = dot(dc, vec3(0.2126, 0.7152, 0.0722));
    dc = mix(vec3(dl), dc, 0.35) * tint; // keep its tones, lose the scan's colour cast
    alb = mix(alb, dc, o);
    nOfs = mix(nOfs, nt, o);
    rough = mix(rough, d.g, o);
    ao *= mix(1.0, d.r, o);
}

void roadShade(Material mat, vec3 p, vec2 rc, vec2 dDx, vec2 dDy, vec3 info, inout vec3 n, float footprint, out vec3 alb, out float rough, out float ao, out float metal) {
    float a = rc.x;          // across (m), left positive
    float v = rc.y;          // along (m)
    float hw = info.x;
    int kind = int(info.y + 0.5);
    float seed = info.z;
    float aa = abs(a);
    float edge = hw - aa;    // distance in from the road edge

    // ---- the asphalt scan, hex-tiled in world space (no repeats along the road)
    vec3 D = gl_WorldRayDirectionEXT;
    float cosv = max(abs(dot(n, D)), 0.02);
    vec3 along = D - n * dot(D, n);
    along = dot(along, along) > 1e-8 ? normalize(along) : vec3(1.0, 0.0, 0.0);
    vec2 gMaj = (along * footprint / cosv).xz, gMin = (cross(n, along) * footprint).xz;
    vec2 nxy;
    float occ, rgh;
    g_proj(mat.texBase, 5, p.xz, gMaj, gMin, alb, nxy, occ, rgh);
    rough = rgh;
    ao = occ;
    metal = 0.0;
    vec2 nOfs = nxy;

    // ---- age: stretches resurfaced at different times (darker, smoother when new; greyer and
    // more open-textured when old), with a long-wave tone change along each stretch
    float seg = floor((v + r_hash1(seed) * 60.0) / mix(35.0, 90.0, r_hash1(seed + 3.0)));
    float age = r_hash1(seg * 7.13 + seed * 3.1);                    // 0 new .. 1 old
    float lum = dot(alb, vec3(0.2126, 0.7152, 0.0722));
    alb = mix(vec3(lum), alb, 0.5);                                   // asphalt is nearly grey
    alb *= mix(0.72, 1.18, age) * mix(0.93, 1.07, g_fbm(p.xz * 0.02 + seed, 2));
    alb = mix(alb, alb * vec3(1.03, 1.0, 0.95), age * 0.6);         // old asphalt bleaches warm
    rough = mix(0.55, 0.92, age) * mix(0.9, 1.05, rgh);
    nOfs *= mix(0.7, 1.25, age);

    // ---- wheel paths: two per lane, polished and darkened by tyres; oil drips between them
    bool twoLane = hw >= 2.6;
    float laneC = twoLane ? hw * 0.5 : 0.0;
    float la = twoLane ? aa - laneC : a;
    float track = exp(-pow((abs(la) - 0.85) / 0.33, 2.0));
    alb *= 1.0 - 0.12 * track;
    rough -= 0.18 * track * (1.0 - age * 0.5);
    float drip = exp(-pow(la / 0.35, 2.0)) * smoothstep(0.55, 0.85, g_fbm(vec2(v * 0.7, a * 3.0) + seed, 3));
    alb *= 1.0 - 0.35 * drip;
    rough -= 0.2 * drip;

    // ---- scanned decals: cracks, repair patches and potholes, more of them on old stretches
    float crack = 0.0;
    if (mat.texEmissive != NO_TEX) {
        int nC = int(mat.emissive.x + 0.5), nP = int(mat.emissive.y + 0.5), nH = int(mat.emissive.z + 0.5);
        const float CELL = 2.5;
        float dc = floor(v / CELL);
        for (int k = -1; k <= 1; ++k) {
            float c = dc + float(k);
            float h = r_hash1(c * 1.37 + seed * 7.7);
            float cage = r_hash1(floor((c * CELL + r_hash1(seed) * 60.0) / mix(35.0, 90.0, r_hash1(seed + 3.0))) * 7.13 + seed * 3.1);
            float pc = 0.3 + 0.6 * cage, pp = 0.05 + 0.1 * cage, ph = 0.02 * cage;
            int idx;
            float size, ang;
            float r1 = r_hash1(c * 2.9 + seed), r2 = r_hash1(c * 4.3 + seed + 1.0), r3 = r_hash1(c * 6.1 + seed + 2.0);
            if (h < ph && nH > 0) {
                idx = nC + nP + int(r1 * float(nH)) % nH;
                size = mat.baseColor.z * mix(0.7, 1.1, r2);
                ang = r3 * 6.2832;
            } else if (h < ph + pp && nP > 0) {
                idx = nC + int(r1 * float(nP)) % nP;
                size = mat.baseColor.y * mix(0.6, 1.2, r2);
                ang = (r3 - 0.5) * 0.3 + (r1 > 0.5 ? 1.5708 : 0.0); // cut square to the road
            } else if (h < ph + pp + pc && nC > 0) {
                idx = int(r1 * float(nC)) % nC;
                size = mat.baseColor.x * mix(1.4, 3.0, r2);
                ang = r3 * 6.2832;
            } else {
                continue;
            }
            // across: anywhere on the road, cracks favour the wheel paths and the centre joint
            float ac = (r_hash1(c * 8.3 + seed + 3.0) - 0.5) * 2.0 * max(hw - size * 0.3, 0.0);
            vec2 cen = vec2(ac, c * CELL + r_hash1(c * 9.7 + seed + 4.0) * CELL);
            r_decal(mat.texEmissive, idx, rc, dDx, dDy, cen, ang, size, mix(0.85, 1.05, cage), alb, nOfs, rough, ao);
        }
    }

    // ---- the edge: crumbling, dusty, with grit and grass creeping in
    // broken bites out of the edge here and there (most of the edge is intact), chunks loosening
    // beside them, dust and grit along the edge
    float bite = pow(g_fbm(vec2(v * 0.35, seed), 3), 3.0) * 1.6 * smoothstep(0.35, 0.8, age + 0.2);
    float chunks = step(0.6, g_fbm(vec2(v * 2.5, a * 2.5) + seed, 3));
    float crumble = max(1.0 - smoothstep(bite * 0.5, bite + 1e-3, edge), chunks * (1.0 - smoothstep(bite, bite + 0.12, edge)) * step(0.05, bite));
    vec3 grit = vec3(0.17, 0.155, 0.13) * mix(0.75, 1.25, g_fbm(p.xz * 9.0, 2));
    if (edge < 0.0) {
        // the worn lip beyond the edge (the mesh slopes down into the shoulder): broken asphalt
        // giving way to the shoulder's gravel (the same scan as the ground there)
        vec3 ga;
        vec2 gn;
        float go, gr;
        g_proj(mat.texBase, 3, p.xz, gMaj, gMin, ga, gn, go, gr);
        float t = smoothstep(0.0, 0.1, -edge + 0.05 * (g_fbm(p.xz * 3.0 + seed, 2) - 0.5));
        alb = mix(mix(alb * 0.85, grit, 0.5), ga * vec3(1.0, 0.93, 0.8), t);
        rough = mix(0.95, gr, t);
        ao = mix(ao, go, t);
        n = normalize(n + vec3(gn.x, 0.0, gn.y) * 0.5 * t);
        return;
    }
    float dust = (1.0 - smoothstep(0.0, 0.6, edge)) * mix(0.3, 1.0, g_fbm(p.xz * 1.5, 2));
    alb = mix(alb, alb * vec3(1.15, 1.1, 1.02), dust * 0.45);
    alb = mix(alb, grit, crumble * 0.85);
    rough = mix(rough, 1.0, max(crumble, dust * 0.4));

    // ---- grooves for the cracks in the normal
    nOfs *= 1.0 - crack * 0.5;

    // ---- manhole covers in the town's streets (every 30-50 m, in a lane)
    if (kind != 1) {
        float mcell = floor(v / 40.0);
        if (r_hash1(mcell * 5.3 + seed) < 0.7) {
            float mv = mcell * 40.0 + 8.0 + r_hash1(mcell + seed + 1.0) * 24.0;
            float ma = (r_hash1(mcell + seed + 2.0) - 0.5) * hw * 0.8;
            float d = length(vec2(v - mv, a - ma));
            if (d < 0.34) {
                float ring = smoothstep(0.30, 0.31, d);
                float pattern = step(0.5, fract((v - mv) * 9.0 + (a - ma) * 9.0)) * step(0.5, fract((v - mv) * 9.0 - (a - ma) * 9.0));
                alb = mix(vec3(0.055, 0.05, 0.045) * mix(0.8, 1.3, pattern), vec3(0.09, 0.085, 0.08), ring);
                rough = mix(0.45, 0.7, pattern);
                metal = 0.65 * (1.0 - ring);
                nOfs = vec2(0.0);
                crack = 0.0;
            } else if (d < 0.37) {
                alb *= 0.6; // the tar ring round the frame
            }
        }
    }

    // ---- road paint, worn: edge lines on two-lane roads and the school road, a dashed centre
    // line on the prefectural road; tyres wear it where they cross it, and it flakes
    float paint = 0.0;
    if (kind == 1 || kind == 3 || (kind == 2 && hw >= 2.4)) {
        paint = max(paint, 1.0 - smoothstep(0.075, 0.075 + footprint * 0.5, abs(aa - (hw - 0.325))));
    }
    if (kind == 1) {
        float dash = step(fract(v / 10.0), 0.5);
        paint = max(paint, dash * (1.0 - smoothstep(0.075, 0.075 + footprint * 0.5, aa)));
    }
    if (paint > 0.0) {
        float flake = smoothstep(0.3, 0.55, g_fbm(p.xz * 6.0 + seed * 3.0, 3) + 0.25 * (1.0 - age));
        float worn = paint * flake * (1.0 - 0.6 * track) * (1.0 - crack);
        alb = mix(alb, vec3(0.78, 0.78, 0.74) * mix(0.85, 1.0, g_fbm(p.xz * 20.0, 2)), worn);
        rough = mix(rough, 0.5, worn);
        nOfs *= 1.0 - 0.8 * worn;
    }

    // ---- a damp patch here and there in the ruts (an April shower)
    float damp = track * smoothstep(0.78, 0.9, g_fbm(p.xz * 0.12 + seed + 40.0, 3)) * (1.0 - paint);
    alb *= 1.0 - 0.45 * damp;
    rough = mix(rough, 0.08, damp);
    nOfs *= 1.0 - damp;

    rough = clamp(rough, 0.05, 1.0);
    vec3 Tg = normalize(vec3(1.0, 0.0, 0.0) - n * n.x);
    vec3 Bg = cross(Tg, n);
    float nz = sqrt(max(1.0 - dot(nOfs, nOfs), 0.05));
    n = normalize(Tg * nOfs.x + Bg * nOfs.y + n * nz);
}

#endif
