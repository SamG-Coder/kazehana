// Ground of a land-use mapped terrain (MAT_GROUND): scanned surfaces chosen by a land-use map,
// blended without visible repetition. Ported from SmartImposter's tri.rchit groundMaterial
// (hex-tile stochastic texturing, contrast-preserving weights, height-based layer blending).
//
// The material's fields carry the ground's data (scene.rs Material::ground):
//   baseColor = (x0, z0, cols, rows): world x / z of the land-use map's first texel, its size
//   texBase   = the land-use map (R8 unorm, one code per metre, read with texelFetch)
//   texNormal = the first of the layer textures: 3 per layer (albedo sRGB, normal, ORM)
#ifndef GROUND_GLSL
#define GROUND_GLSL

#define G_LAYERS 13
// Layers (crates/town GROUND_SCANS): 0 uncut grass, 1 ploughed soil, 2 mud, 3 gravel,
// 4 concrete, 5 asphalt, 6 forest floor, 7 riverbed, 8 wet grass, 9 mossy grass, 10 sparse soil,
// 11 pebbles, 12 dried grass.
// Gains bring the scans' mean albedos to plausible values (they differ several times over).
const float G_GAIN[G_LAYERS] = float[G_LAYERS](1.1, 2.27, 3.0, 1.55, 1.0, 1.18, 0.62, 3.5, 3.5, 3.5, 2.1, 2.42, 1.0);
// Texture repeat in metres: the scans' real size (Megascans surfaces are ~2 x 2 m).
const float G_TILE[G_LAYERS] = float[G_LAYERS](2.0, 2.0, 2.0, 1.5, 1.5, 1.8, 2.0, 2.0, 2.0, 2.0, 2.0, 1.2, 2.0);

float g_hash(vec2 p) { return float(hash2(uvec2(ivec2(floor(p)) + 32768))) * (1.0 / 4294967296.0); }
float g_vnoise(vec2 p) {
    vec2 i = floor(p), f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(mix(g_hash(i), g_hash(i + vec2(1, 0)), u.x), mix(g_hash(i + vec2(0, 1)), g_hash(i + vec2(1, 1)), u.x), u.y);
}
float g_fbm(vec2 p, int oct) {
    float s = 0.0, a = 0.5;
    for (int i = 0; i < oct; ++i) { s += a * g_vnoise(p); p = p * 2.03 + 17.1; a *= 0.5; }
    return s / (1.0 - pow(0.5, float(oct)));
}

struct GHex { vec2 uv[3]; mat2 rot[3]; vec3 w; };

GHex g_hexTile(vec2 uv) {
    GHex h;
    vec2 st = uv * 1.6;
    vec2 sk = vec2(st.x, -0.57735027 * st.x + 1.15470054 * st.y);
    vec2 base = floor(sk);
    vec3 t = vec3(fract(sk), 0.0);
    t.z = 1.0 - t.x - t.y;
    float s = step(0.0, -t.z), s2 = 2.0 * s - 1.0;
    h.w = vec3(-t.z * s2, s - t.y * s2, s - t.x * s2);
    vec2 v[3] = vec2[3](base + vec2(s, s), base + vec2(s, 1.0 - s), base + vec2(1.0 - s, s));
    for (int i = 0; i < 3; ++i) {
        uint r = hash2(uvec2(ivec2(v[i]) + 32768));
        float ang = float(r & 0xFFFFu) * (6.2831853 / 65536.0);
        vec2 off = vec2(float((r >> 16) & 0xFFu), float(r >> 24)) / 256.0;
        float c = cos(ang), sn = sin(ang);
        mat2 R = mat2(c, sn, -sn, c);
        h.uv[i] = R * uv + off * 7.0;
        h.rot[i] = transpose(R);
    }
    return h;
}

vec3 g_hexWeights(vec3 w, vec3 lum) {
    vec3 W = w * w * w * (0.35 + lum);
    W = W * W;
    return W / max(W.x + W.y + W.z, 1e-6);
}

// One scanned layer at p: albedo, tangent-space normal offset (x along +X, y along +Z), occlusion,
// roughness.
// One planar projection of a layer, hex-tiled: pc = coordinates in the projection plane, gMaj /
// gMin = the ray footprint's axes in that plane (anisotropic filtering).
void g_proj(uint ia, int l, vec2 pc, vec2 gMaj, vec2 gMin, out vec3 alb, out vec2 nxy, out float occ, out float rough) {
    float tile = G_TILE[l];
    GHex hx = g_hexTile(pc / tile + float(l) * 0.37);
    vec2 dx[3], dy[3];
    for (int i = 0; i < 3; ++i) {
        mat2 R = transpose(hx.rot[i]);
        dx[i] = R * (gMaj / tile);
        dy[i] = R * (gMin / tile);
    }
    vec3 a[3];
    vec3 lum;
    for (int i = 0; i < 3; ++i) {
        a[i] = textureGrad(textures[nonuniformEXT(ia)], hx.uv[i], dx[i], dy[i]).rgb;
        lum[i] = dot(a[i], vec3(0.3, 0.5, 0.2));
    }
    vec3 w = g_hexWeights(hx.w, lum);
    alb = (a[0] * w.x + a[1] * w.y + a[2] * w.z) * G_GAIN[l];
    if (l == 10) {
        // the soil scan is orange: Japanese verge soil is a grey-brown
        float y = dot(alb, vec3(0.2126, 0.7152, 0.0722));
        alb = mix(vec3(y), alb, 0.35) * vec3(1.06, 1.0, 0.9);
    }
    if (l == 4) {
        // concrete: the asphalt scan's fine grain, lightened and desaturated (a poured slab)
        float y = dot(alb, vec3(0.2126, 0.7152, 0.0722));
        alb = vec3(0.29 + (y - 0.085) * 1.4) * vec3(1.0, 0.99, 0.96);
    }
    nxy = vec2(0.0);
    occ = 0.0;
    rough = 0.0;
    float ws = 0.0;
    for (int i = 0; i < 3; ++i) {
        if (w[i] < 0.01) continue;
        vec2 nt = textureGrad(textures[nonuniformEXT(ia + 1u)], hx.uv[i], dx[i], dy[i]).rg * 2.0 - 1.0;
        nxy += (hx.rot[i] * nt) * w[i];
        vec3 orm = textureGrad(textures[nonuniformEXT(ia + 2u)], hx.uv[i], dx[i], dy[i]).rgb;
        occ += orm.r * w[i];
        rough += orm.g * w[i];
        ws += w[i];
    }
    occ /= max(ws, 1e-4);
    rough /= max(ws, 1e-4);
    if (l == 4) nxy *= 0.5;
}

// One scanned layer at p: triplanar on slopes (a top-down projection smears down revetments and
// banks), plain top-down on level ground. nofs: the normal map's offset in world space.
void g_layer(uint first, int l, vec3 p, vec3 n, vec3 maj, vec3 mnr, out vec3 alb, out vec3 nofs, out float occ, out float rough) {
    uint ia = first + uint(l) * 3u;
    vec3 tw = pow(abs(n), vec3(4.0));
    tw /= tw.x + tw.y + tw.z;
    alb = vec3(0.0);
    nofs = vec3(0.0);
    occ = 0.0;
    rough = 0.0;
    vec3 a;
    vec2 nx;
    float o, r;
    if (tw.y > 0.02) {
        g_proj(ia, l, p.xz, maj.xz, mnr.xz, a, nx, o, r);
        alb += a * tw.y; nofs += vec3(nx.x, 0.0, nx.y) * tw.y; occ += o * tw.y; rough += r * tw.y;
    }
    if (tw.x > 0.02) {
        g_proj(ia, l, p.zy, maj.zy, mnr.zy, a, nx, o, r);
        alb += a * tw.x; nofs += vec3(0.0, nx.y, nx.x) * tw.x; occ += o * tw.x; rough += r * tw.x;
    }
    if (tw.z > 0.02) {
        g_proj(ia, l, p.xy, maj.xy, mnr.xy, a, nx, o, r);
        alb += a * tw.z; nofs += vec3(nx.x, nx.y, 0.0) * tw.z; occ += o * tw.z; rough += r * tw.z;
    }
}

// ---------------------------------------------------------------- grass variance
// Spring flowers in the grass (April): dandelions, white clover, purple henbit. Small dots in a
// jittered grid; where the pixel is wider than a flower they fade to their average colour.
vec3 g_flowers(vec2 xz, float footprint, float density, out float cover) {
    const float CELL = 0.11;
    vec2 c = floor(xz / CELL);
    vec2 f = xz / CELL - c;
    float h = g_hash(c + 71.0);
    float kind = g_hash(c + 13.0);
    vec3 col = kind < 0.55 ? vec3(0.95, 0.72, 0.04) : (kind < 0.85 ? vec3(0.92, 0.92, 0.86) : vec3(0.55, 0.25, 0.55));
    float r = mix(0.12, 0.3, g_hash(c + 5.0));
    vec2 o = vec2(g_hash(c + 2.0), g_hash(c + 9.0)) * (1.0 - 2.0 * r) + r;
    float sharp = 1.0 - smoothstep(r * 0.7, r, length(f - o));
    float avg = 3.14 * r * r;
    float near = smoothstep(CELL * 0.8, CELL * 0.25, footprint);
    cover = step(h, density) * mix(avg, sharp, near);
    return col;
}

// What a land-use code shows: (primary layer, secondary layer, secondary amount, noise scale).
void g_code(uint code, out int a, out int b, out float amt, out float scale) {
    a = 0; b = 12; amt = 0.35; scale = 0.08;
    if (code == 0u) { a = 10; b = 0; amt = 0.35; scale = 0.09; }       // meadow / verge: soil between the real grass, grass-scan patches
    else if (code == 1u) { a = 1; b = 2; amt = 0.35; scale = 0.09; }   // paddy, ploughed in April: soil + wet mud
    else if (code == 2u) { a = 1; b = 10; amt = 0.25; scale = 0.2; }   // vegetable plot
    else if (code == 3u) { a = 3; b = 10; amt = 0.45; scale = 0.22; }  // gravel (farm roads, yards, levee path), soil showing through
    else if (code == 4u) { a = 4; b = 4; amt = 0.0; scale = 0.1; }     // concrete
    else if (code == 5u) { a = 5; b = 3; amt = 0.12; scale = 0.15; }   // asphalt
    else if (code == 6u) { a = 6; b = 9; amt = 0.3; scale = 0.05; }    // forest floor, mossy patches
    else if (code == 7u) { a = 7; b = 11; amt = 0.4; scale = 0.1; }    // riverbed
    else if (code == 8u) { a = 10; b = 8; amt = 0.45; scale = 0.08; }  // levee: soil and wet grass
    else if (code == 9u) { a = 9; b = 10; amt = 0.35; scale = 0.15; }  // garden lawn, worn to soil in places
    else if (code == 10u) { a = 10; b = 3; amt = 0.25; scale = 0.1; }  // school ground (dirt)
    else if (code == 11u) { a = 11; b = 3; amt = 0.25; scale = 0.1; }  // shrine gravel
    else if (code == 12u) { a = 9; b = 6; amt = 0.5; scale = 0.08; }   // moss under the shrine trees
    else if (code == 13u) { a = 3; b = 11; amt = 0.4; scale = 0.3; }   // rail ballast
}

// The ground at world point p (normal n, in/out). footprint: the ray cone's width at the hit.
void groundShade(Material mat, vec3 p, inout vec3 n, float footprint, uint seed, out vec3 albedo, out float rough, out float ao) {
    vec4 ex = mat.baseColor;
    // Land-use boundaries: warped by noise (no pixel staircase), and dithered by a fraction of a
    // metre per sample so that DLSS / TAA blends across them.
    vec2 warp = (vec2(g_fbm(p.xz * 0.3, 3), g_fbm(p.xz * 0.3 + 31.7, 3)) - 0.5) * 1.6;
    uint s = seed;
    // a tent of one texel each way: the stochastic equivalent of bilinear filtering of the codes
    vec2 jit = vec2(rnd(s) + rnd(s), rnd(s) + rnd(s)) - 1.0;
    vec2 q = p.xz + warp + jit - ex.xy;
    ivec2 ij = clamp(ivec2(floor(q)), ivec2(0), ivec2(ex.zw) - 1);
    uint code = uint(texelFetch(textures[nonuniformEXT(mat.texBase)], ij, 0).r * 255.0 + 0.5);

    int la, lb;
    float amt, scale;
    g_code(code, la, lb, amt, scale);
    float mixN = g_fbm(p.xz * scale + 5.3, 4);
    float wb = smoothstep(0.5 - 0.25, 0.5 + 0.25, mixN + (amt - 0.5));
    if (la == lb) wb = 0.0;

    // the footprint on the surface: stretched along the ray's direction by 1 / cos(view angle)
    vec3 D = gl_WorldRayDirectionEXT;
    float cosv = max(abs(dot(n, D)), 0.02);
    vec3 along = D - n * dot(D, n);
    along = dot(along, along) > 1e-8 ? normalize(along) : normalize(cross(n, vec3(0.0, 0.0, 1.0)) + 1e-4);
    vec3 maj = along * footprint / cosv;
    vec3 mnr = cross(n, along) * footprint;
    vec3 aA, aB = vec3(0.0);
    vec3 nA, nB = vec3(0.0);
    float oA, oB = 1.0, rA, rB = 1.0;
    g_layer(mat.texNormal, la, p, n, maj, mnr, aA, nA, oA, rA);
    if (wb > 0.01) g_layer(mat.texNormal, lb, p, n, maj, mnr, aB, nB, oB, rB);
    // height-based blend: the taller (brighter, less occluded) layer wins near the transition
    float hA = dot(aA, vec3(0.3, 0.5, 0.2)) * 3.0 + oA * 0.5;
    float hB = dot(aB, vec3(0.3, 0.5, 0.2)) * 3.0 + oB * 0.5;
    float kA = pow((1.0 - wb) * (hA + 0.4), 3.0), kB = pow(wb * (hB + 0.4), 3.0);
    float k = kB / max(kA + kB, 1e-9);
    albedo = mix(aA, aB, k);
    vec3 nofs = mix(nA, nB, k);
    ao = mix(oA, oB, k);
    rough = mix(rA, rB, k);
    // grass: tones that drift over metres and tens of metres, clover clumps, spring flowers,
    // and dry, worn verges along roads and paths
    if (code == 0u || code == 8u || code == 9u) {
        // how close is a road or a gravel path? (the land-use map around this point)
        float verge = 0.0;
        for (int k = 0; k < 4; ++k) {
            vec2 o = vec2(k == 0 ? 1.6 : (k == 1 ? -1.6 : 0.0), k == 2 ? 1.6 : (k == 3 ? -1.6 : 0.0));
            ivec2 nij = clamp(ivec2(floor(p.xz + o - ex.xy)), ivec2(0), ivec2(ex.zw) - 1);
            uint nc = uint(texelFetch(textures[nonuniformEXT(mat.texBase)], nij, 0).r * 255.0 + 0.5);
            verge += (nc == 5u || nc == 3u || nc == 4u) ? 0.25 : 0.0;
        }
        float t1 = g_fbm(p.xz * 0.45 + 3.0, 3), t2 = g_fbm(p.xz * 0.06 + 11.0, 3);
        vec3 lush = vec3(0.82, 1.04, 0.70), yellow = vec3(1.04, 1.02, 0.76), straw = vec3(1.12, 1.04, 0.84);
        vec3 tint = mix(lush, yellow, smoothstep(0.35, 0.75, t2));
        tint = mix(tint, straw, smoothstep(0.6, 0.85, t1) * 0.6 + verge * 0.45);
        if (code == 9u) tint = mix(tint, lush, 0.5); // gardens are kept greener
        albedo *= mix(vec3(1.0), tint, 0.5);
        // clover: darker, bluer, smoother clumps
        float clover = smoothstep(0.62, 0.72, g_fbm(p.xz * 0.9 + 21.0, 3)) * (1.0 - verge);
        albedo = mix(albedo, albedo * vec3(0.62, 0.82, 0.70), clover * 0.8);
        // verge: trampled, dusty, the soil showing through
        float worn = verge * smoothstep(0.35, 0.65, g_fbm(p.xz * 1.3 + 7.0, 3));
        albedo = mix(albedo, vec3(0.16, 0.14, 0.10) * mix(0.8, 1.2, g_fbm(p.xz * 6.0, 2)), worn * 0.65);
        rough = mix(rough, 1.0, worn);
        nofs *= 1.0 - 0.5 * worn;
        // spring flowers, in drifts
        float fd = smoothstep(0.45, 0.8, g_fbm(p.xz * 0.18 + 33.0, 3)) * 0.15 * (1.0 - worn) * (code == 8u ? 1.3 : 1.0);
        float fc;
        vec3 fcol = g_flowers(p.xz, footprint, fd, fc);
        albedo = mix(albedo, fcol * 0.6, fc);
        nofs *= 1.0 - fc;
    }

    // gravel ground and roadsides: patches of finer pebbles, bare soil, darker damp ground and
    // weeds at the edges, so a strip of gravel never reads as one flat colour
    if (code == 3u || code == 10u) {
        float pat = g_fbm(p.xz * 0.6 + 71.0, 3);
        float big = g_fbm(p.xz * 0.08 + 13.0, 3);
        albedo *= mix(0.78, 1.18, big);
        albedo = mix(albedo, albedo * vec3(1.04, 1.0, 0.9), smoothstep(0.55, 0.75, pat));      // dry, sandy
        float damp = smoothstep(0.62, 0.8, g_fbm(p.xz * 0.25 + 5.0, 3));
        albedo *= 1.0 - 0.3 * damp;                                                              // damp, darker
        rough = mix(rough, 0.75, damp);
        float weed = smoothstep(0.7, 0.82, g_fbm(p.xz * 1.7 + 29.0, 3));
        albedo = mix(albedo, vec3(0.08, 0.11, 0.045), weed * 0.7);                               // weeds
        float stones = step(0.86, g_hash(floor(p.xz * 9.0) + 3.0)) * smoothstep(footprint * 9.0, 0.0, 0.6);
        albedo = mix(albedo, albedo * mix(0.6, 1.4, g_hash(floor(p.xz * 9.0) + 8.0)), stones * 0.6); // the odd bigger stone
    }

    // paddies in April: wet, darker, smoother in the furrows
    if (code == 1u) {
        float wet = smoothstep(0.45, 0.7, g_fbm(p.xz * 0.05 + 2.0, 3));
        albedo *= mix(0.85, 0.6, wet);
        rough = mix(rough, 0.35, wet * 0.8);
    }
    // large-scale variation so the ground does not read as one flat colour from a distance
    float big = g_fbm(p.xz * 0.012 + 9.0, 3);
    albedo *= mix(0.86, 1.12, big);
    nofs -= n * dot(nofs, n);
    n = normalize(n * sqrt(max(1.0 - dot(nofs, nofs), 0.05)) + nofs);
}

#endif
