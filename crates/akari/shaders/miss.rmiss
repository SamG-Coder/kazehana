#version 460
#extension GL_GOOGLE_include_directive : require
#include "rt.glsl"

// Nothing hit: the generation shader shades the sky itself.
layout(location = 0) rayPayloadInEXT Hit pl;

void main() {
    pl.t = -1.0;
}
