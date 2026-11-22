// Thin C ABI over NVIDIA NGX DLSS Ray Reconstruction (DLSS-RR) for Vulkan.
//
// The NGX headers are C++ (overloads, default arguments) with the parameter plumbing in inline
// helpers, so they cannot be called from Rust directly. This file is only compiled when the
// DLSS SDK is found (build.rs, DLSS_SDK=<path to the SDK root with include/ and lib/>). The SDK
// and the nvngx_dlssd.dll runtime are NVIDIA's and are not part of this repository.

#include <vulkan/vulkan.h>
#include <cstdint>
#include <cstring>

#include "nvsdk_ngx_vk.h"
#include "nvsdk_ngx_helpers_vk.h"
#include "nvsdk_ngx_defs_dlssd.h"
#include "nvsdk_ngx_params_dlssd.h"
#include "nvsdk_ngx_helpers_dlssd_vk.h"

namespace {
NVSDK_NGX_Parameter* g_params = nullptr;
NVSDK_NGX_Handle* g_feature = nullptr;
VkDevice g_device = VK_NULL_HANDLE;
bool g_init = false;
// Random project id for this engine (NGX_ENGINE_TYPE_CUSTOM requires a GUID-style id).
const char* kProjectId = "b2a7e4c1-8d3f-4f6a-a1c9-6e5d2b7f3a90";
}

struct SiImage {
    VkImage image;
    VkImageView view;
    VkFormat format;
    uint32_t width;
    uint32_t height;
};

struct SiDlssEval {
    SiImage color;        // noisy HDR radiance, render resolution
    SiImage output;       // denoised + upscaled HDR, output resolution (storage)
    SiImage depth;        // linear view depth, render resolution
    SiImage motion;       // pixel motion (prev - cur), render resolution
    SiImage normal_rough; // world-space normal xyz, roughness w
    SiImage diff_albedo;  // diffuse albedo
    SiImage spec_albedo;  // specular albedo (pre-integrated)
    float jitter_x;       // sub-pixel offset of this frame's samples, render pixels
    float jitter_y;
    uint32_t render_w;
    uint32_t render_h;
    int32_t reset;
    float world_to_view[16]; // row-major, row-vector convention (DirectX style)
    float view_to_clip[16];
    float frame_time_ms;
};

static NVSDK_NGX_Resource_VK res(const SiImage& i, bool rw) {
    VkImageSubresourceRange r = {VK_IMAGE_ASPECT_COLOR_BIT, 0, 1, 0, 1};
    return NVSDK_NGX_Create_ImageView_Resource_VK(i.view, i.image, r, i.format, i.width, i.height, rw);
}

extern "C" {

// Instance / device extensions NGX needs (call before creating the Vulkan instance/device).
int si_dlss_required_extensions(uint32_t* inst_count, const char*** inst_exts, uint32_t* dev_count, const char*** dev_exts) {
    unsigned ic = 0, dc = 0;
    NVSDK_NGX_Result r = NVSDK_NGX_VULKAN_RequiredExtensions(&ic, inst_exts, &dc, dev_exts);
    *inst_count = ic;
    *dev_count = dc;
    return NVSDK_NGX_SUCCEED(r) ? 0 : (int)r;
}

// Initialise NGX and check that Ray Reconstruction is available. dll_dir is searched for
// nvngx_dlssd.dll (besides the executable's folder); data_dir receives NGX logs.
// Returns 0 when DLSS-RR can be used, otherwise an NGX result code (or -2: not available).
int si_dlss_init(VkInstance instance, VkPhysicalDevice pd, VkDevice device, PFN_vkGetInstanceProcAddr gipa,
                 PFN_vkGetDeviceProcAddr gdpa, const wchar_t* dll_dir, const wchar_t* data_dir) {
    NVSDK_NGX_FeatureCommonInfo info;
    std::memset(&info, 0, sizeof(info));
    const wchar_t* paths[1] = {dll_dir};
    info.PathListInfo.Path = paths;
    info.PathListInfo.Length = dll_dir ? 1 : 0;
    info.LoggingInfo.MinimumLoggingLevel = NVSDK_NGX_LOGGING_LEVEL_OFF;
    NVSDK_NGX_Result r = NVSDK_NGX_VULKAN_Init_with_ProjectID(kProjectId, NVSDK_NGX_ENGINE_TYPE_CUSTOM, "1.0", data_dir, instance, pd,
                                                              device, gipa, gdpa, &info, NVSDK_NGX_Version_API);
    if (NVSDK_NGX_FAILED(r)) return (int)r;
    g_init = true;
    g_device = device;
    r = NVSDK_NGX_VULKAN_GetCapabilityParameters(&g_params);
    if (NVSDK_NGX_FAILED(r)) return (int)r;
    int available = 0;
    NVSDK_NGX_Parameter_GetI(g_params, NVSDK_NGX_Parameter_SuperSamplingDenoising_Available, &available);
    return available ? 0 : -2;
}

// Create (or re-create) the Ray Reconstruction feature. quality: 0 DLAA, 1 Quality, 2 Balanced,
// 3 Performance, 4 Ultra Performance. Records into cb; submit it before evaluating.
int si_dlss_create(VkCommandBuffer cb, uint32_t render_w, uint32_t render_h, uint32_t out_w, uint32_t out_h, int quality) {
    if (!g_params) return -1;
    if (g_feature) {
        NVSDK_NGX_VULKAN_ReleaseFeature(g_feature);
        g_feature = nullptr;
    }
    static const NVSDK_NGX_PerfQuality_Value q[5] = {NVSDK_NGX_PerfQuality_Value_DLAA, NVSDK_NGX_PerfQuality_Value_MaxQuality,
                                                      NVSDK_NGX_PerfQuality_Value_Balanced, NVSDK_NGX_PerfQuality_Value_MaxPerf,
                                                      NVSDK_NGX_PerfQuality_Value_UltraPerformance};
    NVSDK_NGX_DLSSD_Create_Params p;
    std::memset(&p, 0, sizeof(p));
    p.InDenoiseMode = NVSDK_NGX_DLSS_Denoise_Mode_DLUnified;
    p.InRoughnessMode = NVSDK_NGX_DLSS_Roughness_Mode_Packed;
    p.InUseHWDepth = NVSDK_NGX_DLSS_Depth_Type_Linear;
    p.InWidth = render_w;
    p.InHeight = render_h;
    p.InTargetWidth = out_w;
    p.InTargetHeight = out_h;
    p.InPerfQualityValue = q[quality < 0 ? 0 : (quality > 4 ? 4 : quality)];
    p.InFeatureCreateFlags = NVSDK_NGX_DLSS_Feature_Flags_IsHDR | NVSDK_NGX_DLSS_Feature_Flags_MVLowRes;
    p.InEnableOutputSubrects = false;
    NVSDK_NGX_Result r = NGX_VULKAN_CREATE_DLSSD_EXT1(g_device, cb, 1, 1, &g_feature, g_params, &p);
    return NVSDK_NGX_SUCCEED(r) ? 0 : (int)r;
}

int si_dlss_evaluate(VkCommandBuffer cb, const SiDlssEval* e) {
    if (!g_feature) return -1;
    NVSDK_NGX_Resource_VK color = res(e->color, false), output = res(e->output, true), depth = res(e->depth, false),
                          motion = res(e->motion, false), normals = res(e->normal_rough, false),
                          diff = res(e->diff_albedo, false), spec = res(e->spec_albedo, false);
    NVSDK_NGX_VK_DLSSD_Eval_Params p;
    std::memset(&p, 0, sizeof(p));
    p.pInColor = &color;
    p.pInOutput = &output;
    p.pInDepth = &depth;
    p.pInMotionVectors = &motion;
    p.pInNormals = &normals;
    p.pInDiffuseAlbedo = &diff;
    p.pInSpecularAlbedo = &spec;
    p.InJitterOffsetX = e->jitter_x;
    p.InJitterOffsetY = e->jitter_y;
    p.InRenderSubrectDimensions.Width = e->render_w;
    p.InRenderSubrectDimensions.Height = e->render_h;
    p.InReset = e->reset;
    p.InMVScaleX = 1.0f;
    p.InMVScaleY = 1.0f;
    p.InPreExposure = 1.0f;
    p.InExposureScale = 1.0f;
    p.pInWorldToViewMatrix = const_cast<float*>(e->world_to_view);
    p.pInViewToClipMatrix = const_cast<float*>(e->view_to_clip);
    p.InFrameTimeDeltaInMsec = e->frame_time_ms;
    NVSDK_NGX_Result r = NGX_VULKAN_EVALUATE_DLSSD_EXT(cb, g_feature, g_params, &p);
    return NVSDK_NGX_SUCCEED(r) ? 0 : (int)r;
}

void si_dlss_shutdown(void) {
    if (g_feature) NVSDK_NGX_VULKAN_ReleaseFeature(g_feature);
    g_feature = nullptr;
    if (g_params) NVSDK_NGX_VULKAN_DestroyParameters(g_params);
    g_params = nullptr;
    if (g_init) NVSDK_NGX_VULKAN_Shutdown1(g_device);
    g_init = false;
}

} // extern "C"
