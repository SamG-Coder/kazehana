#version 460
#extension GL_GOOGLE_include_directive : require
#include "rt.glsl"
#include "foil.glsl"
#include "ground.glsl"
#include "road.glsl"

// Closest hit for every triangle mesh: interpolate the vertex streams, sample the material's
// textures at a ray-cone LOD, apply the normal map, and hand the surface to the ray generation
// shader (which does all the lighting).

layout(location = 0) rayPayloadInEXT Hit pl;
hitAttributeEXT vec2 bary;

#include "imp.glsl"
#include "lawn.glsl"

// UV gradients of the ray footprint's two axes on the surface (anisotropic filtering); zero
// when the mesh has no UVs (then the cone LOD is used).
vec2 gDx = vec2(0.0), gDy = vec2(0.0);
bool gGrad = false;

vec4 tex(uint i, vec2 uv, float lod) {
    if (gGrad) return textureGrad(textures[nonuniformEXT(i)], uv, gDx, gDy);
    return textureLod(textures[nonuniformEXT(i)], uv, lod);
}

void main() {
    MeshInfo m = meshes[gl_InstanceCustomIndexEXT];
    Material mat = materials[m.material];
    uvec3 tri = UVec3Ref(m.idx).v[gl_PrimitiveID];
    vec3 w = vec3(1.0 - bary.x - bary.y, bary.x, bary.y);

    Vec3Ref P = Vec3Ref(m.pos);
    vec3 p0 = P.v[tri.x], p1 = P.v[tri.y], p2 = P.v[tri.z];

    // Object motion: the same triangle and barycentrics on last frame's mesh and transform.
    if (gl_InstanceID < int(motionCount)) {
        InstMotion im = motion[gl_InstanceID];
        if (im.valid != 0u) {
            MeshInfo pm = meshes[im.prevMesh];
            uvec3 ptri = UVec3Ref(pm.idx).v[gl_PrimitiveID];
            Vec3Ref PP = Vec3Ref(pm.pos);
            vec4 op = vec4(PP.v[ptri.x] * w.x + PP.v[ptri.y] * w.y + PP.v[ptri.z] * w.z, 1.0);
            pl.prev = vec4(dot(im.r0, op), dot(im.r1, op), dot(im.r2, op), 1.0);
        }
    }
    // geometric normal in world space
    vec3 e1 = p1 - p0, e2 = p2 - p0;
    mat3 o2w = mat3(gl_ObjectToWorldEXT);
    mat3 nrmM = transpose(inverse(o2w));
    vec3 ngObj = cross(e1, e2);
    vec3 ng = normalize(nrmM * ngObj);

    vec3 n = ng;
    if (m.nrm != 0ul) {
        Vec3Ref N = Vec3Ref(m.nrm);
        n = normalize(nrmM * (N.v[tri.x] * w.x + N.v[tri.y] * w.y + N.v[tri.z] * w.z));
    }

    // Terrain with a land-use map: scanned ground layers (ground.glsl).
    if ((mat.flags & MAT_GROUND) != 0u) {
        if (dot(ng, gl_WorldRayDirectionEXT) > 0.0) { ng = -ng; n = -n; }
        vec3 wp = gl_WorldRayOriginEXT + gl_WorldRayDirectionEXT * gl_HitTEXT;
        vec3 alb;
        float rough, ao, lawnAmt;
        uint seed = hash2(uvec2(gl_LaunchIDEXT.x + gl_LaunchIDEXT.y * 8192u, g.frame.x * 2654435761u + uint(gl_HitTEXT * 997.0)));
        groundShade(mat, wp, n, pl.cone.y + pl.cone.x * gl_HitTEXT, seed, alb, rough, ao, lawnAmt);
        // the lawn (lawn.glsl): on the gardens' lawns, its blades 4 cm high, in the colour of the
        // ground's grass there
        float tHit = gl_HitTEXT;
        uint lawnSlot = uint(g.fog.y + 0.5);
        // the paddies' rice (lawn.glsl, its own baked tile): standing at the stage's height, in
        // its colour (young green, deep green, ripe gold, the stubble's straw)
        uint riceSlot = uint(g.fog.w + 0.5) % 4096u, seedSlot = uint(g.fog.w + 0.5) / 4096u;
        float stR = clamp(mat.metallic, 0.0, 5.0);
        // (until mid-June the young plants: their own tile, a few upright leaves a hill)
        bool young = stR < 1.55 && seedSlot > 0u;
        if (young) riceSlot = seedSlot;
        if (riceSlot > 0u && lawnAmt < 0.0 && g.opts.z == 0u) {
            float st = stR;
            float sunVis;
            bool blade;
            vec3 up = ng.y < 0.0 ? -ng : ng;
            // height, of the baked 0.9 m: seedlings 0.2 m in May, 0.9 m by late July; cut to
            // 0.13 m stubble at the harvest
            float tall = mix(0.42, 1.0, smoothstep(1.55, 2.1, st));
            tall = mix(tall, 0.15, smoothstep(3.3, 3.6, st));
            // the seedlings' tile (0.22 m baked): as planted in May, half as tall again by June
            if (young) tall = mix(0.75, 1.5, smoothstep(1.0, 1.55, st));
            vec3 colour = mix(vec3(0.060, 0.092, 0.024), vec3(0.042, 0.078, 0.018), smoothstep(1.0, 2.0, st));
            colour = mix(colour, vec3(0.16, 0.12, 0.035), smoothstep(2.45, 3.0, st));
            colour = mix(colour, vec3(0.13, 0.105, 0.06), smoothstep(3.3, 3.6, st));
            // as the layer fades out with distance (lawnShade: from 60 % of its reach), the
            // crop's average colour takes its place over the ground, so no edge shows
            {
                float reach = imps[riceSlot - 1u].bmax.x;
                float fadeR = smoothstep(0.6 * reach, reach, tHit) * -lawnAmt;
                float cover = mix(0.3, 0.95, smoothstep(1.0, 2.0, st)) * (1.0 - 0.55 * smoothstep(3.0, 3.5, st));
                alb = mix(alb, mix(alb, colour * 0.85, cover), fadeR);
                rough = mix(rough, mix(rough, 0.85, cover), fadeR);
            }
            if (lawnShade(riceSlot - 1u, wp, up, -lawnAmt, tall, colour, 0.75, tHit, alb, n, ng, ao, rough, sunVis, blade)) {
                pl.sunVis = sunVis;
                if (blade) {
                    pl.t = tHit;
                    pl.albedo = alb;
                    pl.n = n;
                    pl.ng = ng;
                    pl.roughness = rough;
                    pl.metallic = 0.0;
                    pl.emission = vec3(0.0);
                    pl.ao = ao;
                    pl.flags = MAT_FOLIAGE;
                    pl.transmission = 0.3;
                    return;
                }
            }
        }
        if (lawnSlot > 0u && lawnAmt > 0.0 && g.opts.z == 0u) {
            float sunVis;
            bool blade;
            vec3 up = ng.y < 0.0 ? -ng : ng;
            vec3 colour = clamp(alb, vec3(0.02), vec3(0.3));
            if (lawnShade(lawnSlot - 1u, wp, up, lawnAmt, 1.15, colour, 0.0, tHit, alb, n, ng, ao, rough, sunVis, blade)) {
                pl.sunVis = sunVis;
                if (blade) {
                    pl.t = tHit;
                    pl.albedo = alb;
                    pl.n = n;
                    pl.ng = ng;
                    pl.roughness = rough;
                    pl.metallic = 0.0;
                    pl.emission = vec3(0.0);
                    pl.ao = ao;
                    pl.flags = MAT_FOLIAGE;
                    pl.transmission = 0.25;
                    return;
                }
            }
        }
        pl.t = tHit;
        pl.albedo = alb;
        pl.n = n;
        pl.ng = ng;
        // (ground is never glossier than 0.3, but a flooded paddy's still water (rough < 0.05,
        // ground.glsl) is a mirror: the sky and the hills in it)
        pl.roughness = rough < 0.05 ? 0.035 : clamp(rough, 0.3, 1.0);
        pl.metallic = 0.0;
        pl.emission = vec3(0.0);
        pl.ao = ao;
        pl.flags = mat.flags;
        pl.transmission = 0.0;
        return;
    }

    vec2 uv = vec2(0.0);
    float lod = 0.0;
    if (m.uv != 0ul) {
        Vec2Ref U = Vec2Ref(m.uv);
        vec2 u0 = U.v[tri.x], u1 = U.v[tri.y], u2 = U.v[tri.z];
        uv = u0 * w.x + u1 * w.y + u2 * w.z;
        // Ray cones (Akenine-Moller et al., RT Gems ch. 20): texel footprint from the cone width at
        // the hit, the triangle's UV / world area ratio and the view angle. +0.5 log2 of a 2K
        // texture's texel count is folded in per texture below.
        float worldArea = length(cross(o2w * e1, o2w * e2));
        float uvArea = abs((u1.x - u0.x) * (u2.y - u0.y) - (u2.x - u0.x) * (u1.y - u0.y));
        float width = pl.cone.y + pl.cone.x * gl_HitTEXT;
        float cosV = max(abs(dot(gl_WorldRayDirectionEXT, ng)), 0.05);
        lod = 0.5 * log2(max(uvArea, 1e-12) / max(worldArea, 1e-12)) + log2(max(width, 1e-6) / cosV);
        // The footprint is an ellipse on the surface: `width` across the ray, width / cos(view)
        // along it. Map both axes to UV through the triangle (world -> uv Jacobian).
        vec3 w1 = o2w * e1, w2 = o2w * e2;
        vec3 nn = cross(w1, w2);
        float nn2 = max(dot(nn, nn), 1e-20);
        vec3 D = gl_WorldRayDirectionEXT;
        vec3 nu = nn * inversesqrt(nn2);
        vec3 major = D - nu * dot(D, nu);
        major = dot(major, major) > 1e-10 ? normalize(major) : normalize(w1);
        vec3 minor = cross(nu, major);
        major *= width / cosV;
        minor *= width;
        vec2 du1 = u1 - u0, du2 = u2 - u0;
        float a = dot(cross(major, w2), nn) / nn2, b = dot(cross(w1, major), nn) / nn2;
        gDx = a * du1 + b * du2;
        a = dot(cross(minor, w2), nn) / nn2; b = dot(cross(w1, minor), nn) / nn2;
        gDy = a * du1 + b * du2;
        gGrad = true;
    }
    // Asphalt roads: wear, cracks, patches, paint (road.glsl).
    if ((mat.flags & MAT_ROAD) != 0u && m.uv != 0ul && m.col != 0ul) {
        if (dot(ng, gl_WorldRayDirectionEXT) > 0.0) { ng = -ng; n = -n; }
        vec3 wp = gl_WorldRayOriginEXT + gl_WorldRayDirectionEXT * gl_HitTEXT;
        Vec2Ref U = Vec2Ref(m.uv);
        vec2 rc = U.v[tri.x] * w.x + U.v[tri.y] * w.y + U.v[tri.z] * w.z;
        vec3 info = Vec3Ref(m.col).v[tri.x];
        vec3 alb;
        float rough, ao, metal;
        roadShade(mat, wp, rc, gDx, gDy, info, n, pl.cone.y + pl.cone.x * gl_HitTEXT, alb, rough, ao, metal);
        pl.t = gl_HitTEXT;
        pl.albedo = alb;
        pl.n = n;
        pl.ng = ng;
        pl.roughness = rough;
        pl.metallic = metal;
        pl.emission = vec3(0.0);
        pl.ao = ao;
        pl.flags = mat.flags;
        pl.transmission = 0.0;
        return;
    }

    // Per texture: + log2(texture size).
    #define TEX_LOD(i) (lod + log2(float(textureSize(textures[nonuniformEXT(i)], 0).x)))

    vec4 base = mat.baseColor;
    if (mat.texBase != NO_TEX) {
        vec4 t = tex(mat.texBase, uv, TEX_LOD(mat.texBase));
        if ((mat.flags & MAT_RECOLOR) != 0u) t.rgb = vec3(dot(t.rgb, vec3(0.2126, 0.7152, 0.0722)) * 4.0);
        base *= t;
    }
    if (m.col != 0ul) {
        Vec3Ref C = Vec3Ref(m.col);
        base.rgb *= C.v[tri.x] * w.x + C.v[tri.y] * w.y + C.v[tri.z] * w.z;
    }
    float rough = mat.roughness, metal = mat.metallic;
    if (mat.texMr != NO_TEX) {
        vec4 mr = tex(mat.texMr, uv, TEX_LOD(mat.texMr));
        rough *= mr.g;
        metal *= mr.b;
    }
    float ao = 1.0;
    if (mat.texOcclusion != NO_TEX) {
        ao = mix(1.0, tex(mat.texOcclusion, uv, TEX_LOD(mat.texOcclusion)).r, mat.occlusion);
    }
    vec3 emission = mat.emissive;
    bool cardFoil = (mat.flags & MAT_CARD_FOIL) != 0u; // emissive slot holds the foil mask
    if (mat.texEmissive != NO_TEX && !cardFoil) emission *= tex(mat.texEmissive, uv, TEX_LOD(mat.texEmissive)).rgb;

    // Face the ray: both normals on the side the ray came from.
    bool back = dot(ng, gl_WorldRayDirectionEXT) > 0.0;
    if (back) ng = -ng;
    if (dot(n, ng) < 0.0) n = -n;

    if (mat.texNormal != NO_TEX && m.tan != 0ul && m.uv != 0ul) {
        Vec4Ref T = Vec4Ref(m.tan);
        vec4 t4 = T.v[tri.x] * w.x + T.v[tri.y] * w.y + T.v[tri.z] * w.z;
        vec3 t = o2w * t4.xyz;
        t = normalize(t - n * dot(n, t));
        // (a mirrored instance flips the tangent frame's handedness)
        float sgn = (T.v[tri.x].w < 0.0 ? -1.0 : 1.0) * (back ? -1.0 : 1.0) * (determinant(o2w) < 0.0 ? -1.0 : 1.0);
        vec3 b = cross(n, t) * sgn;
        vec2 xy = tex(mat.texNormal, uv, TEX_LOD(mat.texNormal)).rg * 2.0 - 1.0;
        xy *= mat.normalScale;
        vec3 tn = vec3(xy, sqrt(max(1.0 - dot(xy, xy), 0.0)));
        vec3 nn = normalize(t * tn.x + b * tn.y + n * tn.z);
        // keep the mapped normal on the visible side (grazing views)
        float nv = dot(nn, -gl_WorldRayDirectionEXT);
        if (nv < 0.02) nn = normalize(nn + gl_WorldRayDirectionEXT * (nv - 0.02));
        n = nn;
    }

    // Trading-card foil (foil.glsl): view-dependent holographic colour, glitter facets.
    if (cardFoil && mat.texEmissive != NO_TEX && m.tan != 0ul && m.uv != 0ul) {
        vec4 f = tex(mat.texEmissive, uv, TEX_LOD(mat.texEmissive));
        Vec4Ref FT = Vec4Ref(m.tan);
        vec4 ft4 = FT.v[tri.x] * w.x + FT.v[tri.y] * w.y + FT.v[tri.z] * w.z;
        vec3 ft = o2w * ft4.xyz;
        ft = normalize(ft - n * dot(n, ft));
        float fsg = (FT.v[tri.x].w < 0.0 ? -1.0 : 1.0) * (back ? -1.0 : 1.0);
        vec3 fb = cross(n, ft) * fsg;
        vec3 rgb = base.rgb;
        applyCardFoil(f, ft, fb, n, -gl_WorldRayDirectionEXT, rgb, metal, rough);
        base.rgb = rgb;
    }

    // leaves: colour by stand and by tree (not cherry blossom)
    if ((mat.flags & MAT_FOLIAGE) != 0u && (mat.flags & MAT_RECOLOR) == 0u)
        base.rgb *= leafVariation(gl_WorldRayOriginEXT + gl_WorldRayDirectionEXT * gl_HitTEXT);

    // a lawn surface (MAT_LAWN: an artificial turf): the lawn's blades stand on it in its base
    // colour, an even 2.5 cm (lawn.glsl); between them, its backing, darker
    if ((mat.flags & MAT_LAWN) != 0u && g.opts.z == 0u) {
        uint lawnSlot = uint(g.fog.y + 0.5);
        float tHit = gl_HitTEXT, sunVis;
        bool blade;
        vec3 wp = gl_WorldRayOriginEXT + gl_WorldRayDirectionEXT * gl_HitTEXT;
        vec3 colour = mat.baseColor.rgb;
        vec3 alb = base.rgb * 0.55;
        if (lawnSlot > 0u && lawnShade(lawnSlot - 1u, wp, ng, 1.0, 0.75, colour, 1.0, tHit, alb, n, ng, ao, rough, sunVis, blade)) {
            pl.sunVis = sunVis;
            pl.t = tHit;
            pl.albedo = alb;
            pl.n = n;
            pl.ng = ng;
            pl.roughness = blade ? rough : 0.9;
            pl.metallic = 0.0;
            pl.emission = vec3(0.0);
            pl.ao = ao;
            pl.flags = blade ? MAT_FOLIAGE : 0u;
            pl.transmission = blade ? 0.2 : 0.0;
            return;
        }
    }

    pl.t = gl_HitTEXT;
    pl.albedo = base.rgb;
    pl.n = n;
    pl.ng = ng;
    pl.roughness = clamp(rough, 0.02, 1.0);
    pl.metallic = clamp(metal, 0.0, 1.0);
    pl.emission = emission;
    pl.ao = ao;
    pl.flags = mat.flags;
    pl.transmission = mat.transmission;
}
