#version 450
#include "scene.glsl"

layout(location = 0) in vec2 v_ndc;
layout(location = 0) out vec4 out_color;

float hash(vec3 p) {
    p = fract(p * 0.3183099 + 0.1);
    p *= 17.0;
    return fract(p.x * p.y * p.z * (p.x + p.y + p.z));
}

// Value noise for soft clouds (to be replaced by the SmartImposter cloud model).
float noise(vec3 x) {
    vec3 i = floor(x);
    vec3 f = fract(x);
    f = f * f * (3.0 - 2.0 * f);
    return mix(mix(mix(hash(i), hash(i + vec3(1, 0, 0)), f.x), mix(hash(i + vec3(0, 1, 0)), hash(i + vec3(1, 1, 0)), f.x), f.y),
               mix(mix(hash(i + vec3(0, 0, 1)), hash(i + vec3(1, 0, 1)), f.x), mix(hash(i + vec3(0, 1, 1)), hash(i + vec3(1, 1, 1)), f.x), f.y), f.z);
}

float fbm(vec3 p) {
    float a = 0.5, s = 0.0;
    for (int i = 0; i < 5; i++) {
        s += a * noise(p);
        p *= 2.03;
        a *= 0.5;
    }
    return s;
}

void main() {
    vec4 far = scene.inv_view_proj * vec4(v_ndc, 1.0, 1.0);
    vec4 near = scene.inv_view_proj * vec4(v_ndc, 0.0, 1.0);
    vec3 dir = normalize(far.xyz / far.w - near.xyz / near.w);

    vec3 c = sky_color(dir);

    // Sun disc.
    vec3 sun = normalize(scene.sun_dir.xyz);
    float sd = dot(dir, sun);
    c += scene.sun_color.rgb * smoothstep(0.9996, 0.9999, sd) * 40.0 * step(-0.02, sun.y);

    // Moon disc with phase shading.
    vec3 moon = normalize(scene.moon_dir.xyz);
    float md = dot(dir, moon);
    c += vec3(0.9, 0.92, 1.0) * smoothstep(0.9997, 0.99985, md) * (0.4 + 3.0 * scene.moon_dir.w) * step(0.0, moon.y);

    // Stars.
    float stars = 0.0;
    if (dir.y > 0.0) {
        vec3 cell = floor(dir * 300.0);
        float h = hash(cell);
        stars = step(0.997, h) * smoothstep(0.0, 0.2, dir.y) * (0.5 + 0.5 * sin(scene.camera_pos.w * 2.0 + h * 50.0));
    }
    c += vec3(stars) * scene.params.w * 2.0;

    // Clouds on a virtual plane 1.5 km up, drifting slowly.
    if (dir.y > 0.01) {
        vec2 uv = dir.xz / dir.y * 1.5 + vec2(scene.camera_pos.w * 0.004, 0.0);
        float d = smoothstep(0.48, 0.75, fbm(vec3(uv * 0.9, scene.camera_pos.w * 0.002)));
        float lit = max(scene.sun_dir.w, 0.0);
        vec3 cloud = scene.sun_color.rgb * lit * 0.9 + scene.ambient.rgb * 1.6 + vec3(0.02, 0.025, 0.04) * scene.moon_dir.w * 8.0;
        c = mix(c, cloud, d * smoothstep(0.01, 0.15, dir.y) * 0.85);
    }

    out_color = vec4(tonemap(c), 1.0);
}
