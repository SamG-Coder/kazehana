#version 460
#extension GL_GOOGLE_include_directive : require
#include "rt.glsl"
#include "imp.glsl"

// Closest hit for an impostor: the baked view's albedo and normal, as a surface for the path
// tracer (leaves: two-sided, translucent foliage).

layout(location = 0) rayPayloadInEXT Hit pl;
hitAttributeEXT ImpHit hit;

void main() {
    PlantRec pr = plants[gl_InstanceCustomIndexEXT + gl_PrimitiveID];
    ImpInfo info = imps[plantVariant(pr)];
    vec4 a = unpackUnorm4x8(hit.albedo);
    vec3 alb = a.rgb * a.rgb; // the atlas stores sqrt(albedo)
    float leafy = info.bmin.w > 0.5 ? 1.0 : smoothstep(0.3, 0.7, a.a);
    vec3 n = normalize(mat3(gl_ObjectToWorldEXT) * (transpose(inverse(plantMatrix(pr))) * octDecode(unpackSnorm2x16(hit.normal))));
    if (dot(n, gl_WorldRayDirectionEXT) > 0.0) n = -n;
    // leaf colour by stand and by tree, as the meshes (rt.glsl leafVariation)
    alb *= mix(vec3(1.0), leafVariation(gl_WorldRayOriginEXT + gl_WorldRayDirectionEXT * gl_HitTEXT), leafy);
    pl.t = gl_HitTEXT;
    pl.albedo = alb;
    pl.n = n;
    pl.ng = n;
    pl.roughness = mix(0.85, 0.6, leafy);
    pl.metallic = 0.0;
    pl.emission = vec3(0.0);
    pl.ao = 1.0;
    pl.flags = leafy > 0.5 ? (MAT_FOLIAGE | MAT_DOUBLE_SIDED) : 0u;
    pl.transmission = leafy * info.frame.z;
}
