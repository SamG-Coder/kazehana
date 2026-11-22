#version 460
#extension GL_EXT_ray_tracing : require

// Shadow / visibility rays: reaching here means nothing was in the way.
layout(location = 1) rayPayloadInEXT uint vis;

void main() {
    vis = 1u;
}
