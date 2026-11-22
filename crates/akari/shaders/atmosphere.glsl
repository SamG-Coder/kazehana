// Physically based atmosphere (globe mode, globe.rchit / main.rgen). Needs PI (shared.glsl).
#ifndef ATMOSPHERE_GLSL
#define ATMOSPHERE_GLSL

// ---- atmosphere (single scattering, Rayleigh + Mie, ray marched): works from the ground to orbit
const float R_GROUND = 6371000.0;
const float R_TOP = 6471000.0;
const vec3 BETA_R = vec3(5.8e-6, 13.5e-6, 33.1e-6);
const float BETA_M = 21e-6;
const float H_R = 8000.0;
const float H_M = 1200.0;
const float MIE_G = 0.76;

// Ray / sphere (centred at the origin): (near, far), far < 0 if missed.
vec2 raySphere(vec3 ro, vec3 rd, float r) {
    float b = dot(ro, rd);
    float c = dot(ro, ro) - r * r;
    float disc = b * b - c;
    if (disc < 0.0) return vec2(1e30, -1.0);
    float s = sqrt(disc);
    return vec2(-b - s, -b + s);
}

// Optical depth (Rayleigh, Mie) from x towards the sun to the top of the atmosphere; huge if the
// planet blocks the sun.
vec2 lightDepth(vec3 x, vec3 L) {
    vec2 g = raySphere(x, L, R_GROUND);
    if (g.x > 0.0 && g.y > 0.0) return vec2(1e9); // (a miss is (1e30, -1): not blocked)
    float t1 = raySphere(x, L, R_TOP).y;
    const int N = 6;
    float ds = t1 / float(N);
    vec2 od = vec2(0.0);
    for (int i = 0; i < N; ++i) {
        float h = length(x + L * ((float(i) + 0.5) * ds)) - R_GROUND;
        od += vec2(exp(-h / H_R), exp(-h / H_M)) * ds;
    }
    return od;
}

// Ozone absorbs orange and red light (Chappuis band): over the long paths near the horizon it
// keeps the sky pale blue-white instead of yellow-brown (single scattering alone yellows it).
// Its 25 km layer is folded into the Rayleigh profile: a vertical column of ~0.03 (green).
const vec3 BETA_O = vec3(0.650e-6, 1.881e-6, 0.085e-6) * 2.0;

vec3 extinction(vec2 od) {
    return exp(-((BETA_R + BETA_O) * od.x + BETA_M * 1.11 * od.y));
}

// In-scattered light along ro + rd * [0, tmax] (ro relative to the planet centre) and the
// transmittance of that segment.
vec3 atmosphere(vec3 ro, vec3 rd, float tmax, vec3 L, float sunI, out vec3 T) {
    T = vec3(1.0);
    vec2 a = raySphere(ro, rd, R_TOP);
    if (a.y < 0.0) return vec3(0.0);
    float t0 = max(a.x, 0.0);
    float t1 = min(a.y, tmax);
    vec2 gnd = raySphere(ro, rd, R_GROUND);
    if (gnd.x > 0.0) t1 = min(t1, gnd.x);
    if (t1 <= t0) return vec3(0.0);
    // Samples packed towards the ray's lower (denser) end, quadratically: a ray along the ground
    // crosses hundreds of km of air and gathers most of its light in the first tens of km; a ray
    // from orbit down to the ground meets the dense air at its far end. Each sample is dimmed by
    // the air up to its own middle, not to the end of its step (that turned the horizon brown: a
    // 50 km step of sea-level air holds an optical depth of ~1.6 in blue).
    const int N = 20;
    float len = t1 - t0;
    bool denseEnd = length(ro + rd * t1) < length(ro + rd * t0);
    vec2 od = vec2(0.0);
    vec3 sumR = vec3(0.0), sumM = vec3(0.0);
    for (int i = 0; i < N; ++i) {
        float ua = float(i) / float(N), ub = float(i + 1) / float(N);
        float sa = denseEnd ? 1.0 - (1.0 - ua) * (1.0 - ua) : ua * ua;
        float sb = denseEnd ? 1.0 - (1.0 - ub) * (1.0 - ub) : ub * ub;
        float ta = t0 + len * sa, tb = t0 + len * sb;
        float ds = tb - ta;
        vec3 x = ro + rd * (0.5 * (ta + tb));
        float h = max(length(x) - R_GROUND, 0.0);
        vec2 d = vec2(exp(-h / H_R), exp(-h / H_M)) * ds;
        vec3 tr = extinction(od + 0.5 * d + lightDepth(x, L));
        od += d;
        sumR += d.x * tr;
        sumM += d.y * tr;
    }
    float mu = dot(rd, L);
    float pR = 3.0 / (16.0 * PI) * (1.0 + mu * mu);
    float g2 = MIE_G * MIE_G;
    float pM = 3.0 / (8.0 * PI) * ((1.0 - g2) * (1.0 + mu * mu)) / ((2.0 + g2) * pow(1.0 + g2 - 2.0 * MIE_G * mu, 1.5));
    T = extinction(od);
    return sunI * (sumR * BETA_R * pR + sumM * BETA_M * pM);
}

#endif
