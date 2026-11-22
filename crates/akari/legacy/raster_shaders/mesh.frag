#version 450
#include "scene.glsl"

layout(push_constant) uniform Push {
    mat4 model;
    vec4 tint;
} pc;

layout(location = 0) in vec3 v_world;
layout(location = 1) in vec3 v_normal;
layout(location = 2) in vec3 v_color;

layout(location = 0) out vec4 out_color;

void main() {
    vec3 n = normalize(v_normal);
    vec3 albedo = v_color;
    vec3 to_cam = scene.camera_pos.xyz - v_world;
    float dist = length(to_cam);

    // Sun and moon (wrap lighting keeps blockout shapes readable).
    vec3 l_sun = normalize(scene.sun_dir.xyz);
    float ndl = max(dot(n, l_sun), 0.0);
    vec3 light = scene.sun_color.rgb * ndl * scene.sun_dir.w;
    vec3 l_moon = normalize(scene.moon_dir.xyz);
    light += vec3(0.55, 0.65, 1.0) * max(dot(n, l_moon), 0.0) * scene.moon_dir.w;

    // Hemisphere ambient: sky from above, ground bounce from below.
    float up = n.y * 0.5 + 0.5;
    vec3 ground = scene.ambient.rgb * scene.ambient.w;
    light += mix(ground, scene.ambient.rgb, up);
    // Cheap height-based occlusion near the floor.
    light *= mix(0.75, 1.0, clamp(v_world.y * 2.0, 0.0, 1.0));

    // Point lights: street lamps, windows, vending machines.
    int count = int(scene.params.z);
    for (int i = 0; i < count; i++) {
        vec3 d = scene.light_pos[i].xyz - v_world;
        float r = scene.light_pos[i].w;
        float dd = dot(d, d);
        float att = clamp(1.0 - dd / (r * r), 0.0, 1.0);
        att = att * att / (1.0 + dd);
        light += scene.light_color[i].rgb * att * (max(dot(n, d * inversesqrt(dd)), 0.0) * 0.8 + 0.2);
    }

    vec3 c = albedo * light + albedo * pc.tint.w;

    // Distance fog towards the sky colour in that direction.
    float f = 1.0 - exp(-dist * scene.fog.w);
    vec3 fog_col = mix(scene.fog.rgb, sky_color(-normalize(to_cam)), 0.5);
    c = mix(c, fog_col, f);

    out_color = vec4(tonemap(c), 1.0);
}
