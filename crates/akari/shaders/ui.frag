#version 450
// The atlas (glyphs: white with their coverage in alpha; the white block for plain rects; images
// in their own colours) times the vertex colour. Blending happens in display (sRGB-encoded)
// space, like any 2D UI.
layout(set = 0, binding = 0) uniform sampler2D atlas;
layout(location = 0) in vec2 vUv;
layout(location = 1) in vec4 vColor;
layout(location = 0) out vec4 outColor;
void main() {
    vec4 t = texture(atlas, vUv);
    outColor = vec4(vColor.rgb * t.rgb, vColor.a * t.a);
}
