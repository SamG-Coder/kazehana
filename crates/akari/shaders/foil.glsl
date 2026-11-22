// Trading-card foil (MAT_CARD_FOIL): the holographic colour shift of Mythbeasts cards.
//
// The foil mask (tools/cardgen foil.png, RGBA unorm) is bound in the material's emissive
// texture slot: R strength, G pattern id * 20, B phase, A glitter seed. The closest-hit shader
// turns it into a view-dependent metallic surface: the diffraction colour follows the view
// direction in the card's tangent plane, so the rainbow sweeps as the card tilts, while the path
// tracer supplies the real highlights (lamp, sky, reflections). Glitter flecks tilt the shading
// normal so they flash when they line up with a light. Mirrors docs/CARD_TEMPLATES.md and
// tools/cardgen/src/shade.rs foil_color.
#ifndef FOIL_GLSL
#define FOIL_GLSL

#define FOIL_AURORA 1
#define FOIL_SHATTER 2
#define FOIL_ETCH 3
#define FOIL_GILT 4
#define FOIL_SILVER 5
#define FOIL_COSMOS 6
#define FOIL_ICE 7
#define FOIL_RAINBOW 8
#define FOIL_GLASS 9
#define FOIL_SAKURA 10
#define FOIL_OBSIDIAN 11
#define FOIL_TAU 6.28318530718

vec3 foilSpectral(float h) {
    h = fract(h) * 6.0;
    return clamp(vec3(abs(h - 3.0) - 1.0, 2.0 - abs(h - 2.0), 2.0 - abs(h - 4.0)), 0.0, 1.0);
}

float foilLuma(vec3 c) { return dot(c, vec3(0.2126, 0.7152, 0.0722)); }

vec3 foilColor(int id, float phase, float spark, float ang, float lobe) {
    vec3 silver = vec3(0.55);
    vec3 c = vec3(0.0);
    if (id == FOIL_AURORA) c = mix(silver, foilSpectral(phase + ang * 1.1), 0.30 + 0.40 * lobe) * (0.55 + 1.0 * lobe);
    else if (id == FOIL_SHATTER) {
        float fl = 0.5 + 0.5 * sin(FOIL_TAU * (phase * 5.0 + ang * 2.0));
        c = mix(silver, foilSpectral(phase * 0.15 + ang * 1.1), 0.30 + 0.35 * lobe) * (0.45 + 0.8 * lobe + 0.35 * fl * fl);
    } else if (id == FOIL_ETCH) c = mix(silver, foilSpectral(phase + ang * 1.2), 0.30 + 0.30 * lobe) * (0.5 + 1.0 * lobe);
    else if (id == FOIL_GILT) {
        float b = 0.5 + 0.5 * sin(FOIL_TAU * (ang * 1.4 + phase));
        c = vec3(1.0, 0.74, 0.36) * (0.30 + 0.55 * b * b + 1.1 * lobe) + foilSpectral(ang + phase) * 0.05;
    } else if (id == FOIL_SILVER) c = vec3(0.45 + 1.0 * lobe);
    else if (id == FOIL_COSMOS) {
        vec3 neb = mix(foilSpectral(0.62 + phase * 0.5 + ang * 0.7), vec3(0.25, 0.2, 0.55), 0.35);
        c = mix(vec3(0.35), neb, 0.55 + 0.3 * lobe) * (0.5 + 1.1 * lobe);
    } else if (id == FOIL_ICE) {
        float fl = 0.5 + 0.5 * sin(FOIL_TAU * (phase * 7.0 + ang * 2.4));
        c = mix(vec3(0.78, 0.9, 1.0), foilSpectral(phase + ang * 0.6), 0.25) * (0.3 + 0.55 * fl * fl + 0.9 * lobe);
    } else if (id == FOIL_RAINBOW) c = mix(vec3(0.7), foilSpectral(phase + ang * 0.9), 0.55 + 0.25 * lobe) * (0.6 + 0.9 * lobe);
    else if (id == FOIL_GLASS) {
        float sh = 0.5 + 0.5 * sin(FOIL_TAU * (phase * 4.0 + ang * 1.5));
        c = mix(foilSpectral(phase), vec3(1.0), 0.6) * (0.45 + 0.6 * sh + 0.8 * lobe);
    } else if (id == FOIL_SAKURA) {
        float b = 0.5 + 0.5 * sin(FOIL_TAU * (ang * 1.2 + phase));
        c = vec3(1.0, 0.72, 0.74) * (0.45 + 0.45 * b + 1.0 * lobe) + foilSpectral(ang * 0.8 + phase) * 0.08;
    } else if (id == FOIL_OBSIDIAN) c = vec3(0.05) + mix(vec3(0.6), foilSpectral(phase + ang * 0.8), 0.5) * 1.6 * pow(lobe, 60.0);
    if (spark > 0.0) c += vec3(pow(max(cos(FOIL_TAU * (spark * 13.0 + ang * 2.5)), 0.0), 60.0) * 1.2);
    return c;
}

// Apply the foil at a hit. `f` is the foil texel, `base` the printed colour; t/b/n the tangent
// frame, v the direction towards the viewer. Updates albedo, metallic, roughness and normal.
void applyCardFoil(vec4 f, vec3 t, vec3 b, inout vec3 n, vec3 v, inout vec3 albedo, inout float metal, inout float rough) {
    float strength = f.r;
    if (strength < 0.004) return;
    int id = int(round(f.g * 255.0 / 20.0));
    float phase = f.b;
    float spark = f.a;
    // view direction in the card's plane: tilting the card moves this, sweeping the colour
    vec2 vt = vec2(dot(v, t), dot(v, b));
    float ang = (vt.x * 0.8 + vt.y * 0.6) * 3.2;
    // the highlight itself comes from the path tracer; this lobe term only shapes the tint
    float lobe = 0.55;
    vec3 fc = foilColor(id, phase, spark, ang, lobe);
    vec3 foiled;
    if (id == FOIL_GILT || id == FOIL_SILVER || id == FOIL_SAKURA) foiled = fc * (0.22 + 1.5 * foilLuma(albedo));
    else if (id == FOIL_OBSIDIAN) foiled = fc + albedo * 0.6;
    else foiled = albedo * fc * 1.3 + fc * 0.12 * sqrt(foilLuma(albedo));
    // glitter flecks: tilted micro-facets that flash when they face a light
    if (spark > 0.0) {
        float a = spark * FOIL_TAU * 13.0;
        n = normalize(n + (t * cos(a) + b * sin(a)) * (0.16 * strength));
    }
    albedo = mix(albedo, clamp(foiled, 0.0, 1.0), strength);
    metal = mix(metal, id == FOIL_OBSIDIAN ? 0.7 : 0.85, strength);
    rough = mix(rough, id == FOIL_ETCH ? 0.2 : 0.12, strength);
}

#endif
