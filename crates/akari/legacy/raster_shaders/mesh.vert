#version 450
#include "scene.glsl"

layout(push_constant) uniform Push {
    mat4 model;
    vec4 tint; // srgb rgb, w = emissive strength
} pc;

layout(location = 0) in vec3 in_pos;
layout(location = 1) in vec3 in_normal;
layout(location = 2) in vec3 in_color;

layout(location = 0) out vec3 v_world;
layout(location = 1) out vec3 v_normal;
layout(location = 2) out vec3 v_color;

void main() {
    vec4 w = pc.model * vec4(in_pos, 1.0);
    v_world = w.xyz;
    v_normal = mat3(pc.model) * in_normal;
    v_color = srgb_to_linear(in_color * pc.tint.rgb);
    gl_Position = scene.view_proj * w;
}
