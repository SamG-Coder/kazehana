// Shared declarations for Akari's ray tracing stages and compute passes.
#ifndef COMMON_GLSL
#define COMMON_GLSL

#define PI 3.14159265
#define INV_PI 0.31830989

// ---------------------------------------------------------------- per-frame constants
// Mirrors `Globals` in src/renderer.rs (std140: vec4s only).
struct Globals {
    vec4 camPos;    // xyz, w = seconds (animation clock)
    vec4 camRight;  // xyz, w = tan(fov/2) * aspect
    vec4 camUp;     // xyz, w = tan(fov/2)
    vec4 camFwd;    // xyz, w = pixel spread angle (rad) for texture LOD
    vec4 prevPos;   // previous frame's camera, same layout (TAA / DLSS motion)
    vec4 prevRight;
    vec4 prevUp;
    vec4 prevFwd;
    vec4 sunDir;    // towards the sun; w = sun illuminance scale (0 when the sun is down)
    vec4 moonDir;   // towards the moon; w = moon illuminance scale (phase, altitude)
    vec4 sky;       // x camera altitude above sea level (m), y star brightness, z ground albedo, w exposure compensation (EV)
    vec4 fog;       // x aerial haze density scale (1 = clear day), y-w unused
    uvec4 frame;    // x frame counter, y frames the camera has been still, z light count, w debug view
    uvec4 opts;     // x max bounces, y 1 = write DLSS guides, z AO-only fallback, w unused
};

// Sub-pixel jitter for TAA / DLSS: Halton(2,3), 32-frame cycle, in [0,1)^2 (dlss.rs jitter).
float halton(uint i, uint b) {
    float f = 1.0, r = 0.0;
    while (i > 0u) {
        f /= float(b);
        r += f * float(i % b);
        i /= b;
    }
    return r;
}
vec2 taaJitter(uint frame) { uint i = (frame % 32u) + 1u; return vec2(halton(i, 2u), halton(i, 3u)); }

// ---------------------------------------------------------------- random numbers
uint pcg(uint v) {
    uint state = v * 747796405u + 2891336453u;
    uint word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}
uint hash2(uvec2 v) { return pcg(v.x ^ pcg(v.y + 0x9e3779b9u)); }
float rnd(inout uint s) { s = pcg(s); return float(s) * (1.0 / 4294967296.0); }

// ---------------------------------------------------------------- mappings
float signNZ(float v) { return v >= 0.0 ? 1.0 : -1.0; }
// Full-sphere octahedral encoding, Y as the pole. Result in [-1,1]^2.
vec2 octEncode(vec3 n) {
    n /= (abs(n.x) + abs(n.y) + abs(n.z));
    vec2 p = n.xz;
    if (n.y < 0.0) p = (1.0 - abs(p.yx)) * vec2(signNZ(p.x), signNZ(p.y));
    return p;
}
vec3 octDecode(vec2 e) {
    vec3 v = vec3(e.x, 1.0 - abs(e.x) - abs(e.y), e.y);
    if (v.y < 0.0) {
        vec2 p = (1.0 - abs(v.zx)) * vec2(signNZ(v.x), signNZ(v.z));
        v.x = p.x;
        v.z = p.y;
    }
    return normalize(v);
}

// Orthonormal basis around n (Duff et al. 2017).
void basis(vec3 n, out vec3 t, out vec3 b) {
    float s = n.z >= 0.0 ? 1.0 : -1.0;
    float a = -1.0 / (s + n.z);
    float c = n.x * n.y * a;
    t = vec3(1.0 + s * n.x * n.x * a, s * c, -s * n.x);
    b = vec3(c, s + n.y * n.y * a, -n.y);
}

float luminance(vec3 c) { return dot(c, vec3(0.2126, 0.7152, 0.0722)); }

// Pixel position of world point x (or a direction, w = 0) for a camera given as pos/right/up/fwd.
vec2 projectPx(vec4 x, vec4 pos, vec4 right, vec4 up, vec4 fwd, vec2 size) {
    vec3 v = x.w > 0.5 ? x.xyz - pos.xyz : x.xyz;
    float z = max(dot(v, fwd.xyz), 1e-4);
    vec2 ndc = vec2(dot(v, right.xyz) / (z * right.w), -dot(v, up.xyz) / (z * up.w));
    return (ndc * 0.5 + 0.5) * size;
}

#endif
