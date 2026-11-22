// Per-frame scene data shared by every shader. Mirrors `SceneUniforms` in src/renderer.rs.

#define MAX_LIGHTS 32

layout(set = 0, binding = 0) uniform Scene {
    mat4 view_proj;
    mat4 inv_view_proj;
    vec4 camera_pos;     // xyz, w = time in seconds
    vec4 sun_dir;        // towards the sun, w = intensity
    vec4 sun_color;      // linear rgb
    vec4 moon_dir;       // towards the moon, w = intensity
    vec4 sky_zenith;     // linear rgb
    vec4 sky_horizon;    // linear rgb
    vec4 ambient;        // linear rgb sky light, w = ground bounce
    vec4 fog;            // linear rgb, w = density (1/m)
    vec4 params;         // x = night factor 0..1, y = exposure, z = light count, w = star visibility
    vec4 light_pos[MAX_LIGHTS];   // xyz, w = radius
    vec4 light_color[MAX_LIGHTS]; // linear rgb * intensity
} scene;

vec3 srgb_to_linear(vec3 c) {
    return pow(c, vec3(2.2));
}

// ACES filmic approximation (Narkowicz).
vec3 tonemap(vec3 x) {
    x *= scene.params.y;
    return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), 0.0, 1.0);
}

vec3 sky_color(vec3 dir) {
    float up = clamp(dir.y, -1.0, 1.0);
    float h = pow(1.0 - max(up, 0.0), 3.0);
    vec3 c = mix(scene.sky_zenith.rgb, scene.sky_horizon.rgb, h);
    // Below the horizon fade to a darker haze.
    c = mix(c, scene.sky_horizon.rgb * 0.45, smoothstep(0.0, -0.25, up));
    // Warm glow around the sun near sunrise/sunset.
    float s = max(dot(dir, normalize(scene.sun_dir.xyz)), 0.0);
    c += scene.sun_color.rgb * (pow(s, 8.0) * 0.25 + pow(s, 64.0) * 0.6) * min(scene.sun_dir.w, 1.0);
    return c;
}
