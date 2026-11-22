#version 450
// 2D overlay (ui.rs): pixel positions -> clip space.
layout(push_constant) uniform Push { vec2 screen; } pc;
layout(location = 0) in vec2 inPos;
layout(location = 1) in vec2 inUv;
layout(location = 2) in vec4 inColor;
layout(location = 0) out vec2 vUv;
layout(location = 1) out vec4 vColor;
void main() {
    vUv = inUv;
    vColor = inColor;
    gl_Position = vec4(inPos / pc.screen * 2.0 - 1.0, 0.0, 1.0);
}
