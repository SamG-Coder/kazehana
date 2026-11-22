//! Compiles shaders/ to SPIR-V with glslangValidator from the Vulkan SDK, and (when DLSS_SDK
//! points at the NVIDIA DLSS SDK) the DLSS Ray Reconstruction shim in native/.

use std::{env, path::PathBuf, process::Command};

fn main() {
    build_dlss();
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let glslang = env::var("VULKAN_SDK")
        .ok()
        .map(|s| PathBuf::from(s).join("Bin").join("glslangValidator.exe"))
        .filter(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from("glslangValidator"));
    println!("cargo:rerun-if-changed=shaders");
    for entry in std::fs::read_dir("shaders").expect("shaders dir") {
        let path = entry.unwrap().path();
        println!("cargo:rerun-if-changed={}", path.display());
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !matches!(ext, "rgen" | "rmiss" | "rchit" | "rahit" | "rint" | "comp" | "vert" | "frag") {
            continue;
        }
        let name = path.file_name().unwrap().to_str().unwrap();
        compile(&glslang, &path, &out.join(format!("{name}.spv")), &[]);
        if ext == "rgen" {
            // Shader Execution Reordering variant (used when the device supports it).
            compile(&glslang, &path, &out.join(format!("{}_ser.rgen.spv", name.trim_end_matches(".rgen"))), &["-DSER"]);
        }
    }
}

fn compile(glslang: &PathBuf, src: &PathBuf, dst: &PathBuf, defines: &[&str]) {
    let result = Command::new(glslang)
        .args(["--target-env", "vulkan1.3", "-V", "-Ishaders"])
        .args(defines)
        .arg(src)
        .arg("-o")
        .arg(dst)
        .output()
        .expect("failed to run glslangValidator (is the Vulkan SDK installed?)");
    if !result.status.success() {
        panic!(
            "shader {} failed to compile:\n{}{}",
            src.display(),
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
}

/// DLSS Ray Reconstruction is optional: with DLSS_SDK set to the SDK root (include/, lib/), the
/// C++ shim is compiled and linked against NGX and the `dlss` cfg is enabled.
fn build_dlss() {
    println!("cargo:rustc-check-cfg=cfg(dlss)");
    println!("cargo:rerun-if-env-changed=DLSS_SDK");
    println!("cargo:rerun-if-changed=native/dlss_shim.cpp");
    let Some(sdk) = env::var("DLSS_SDK").ok().map(PathBuf::from).filter(|p| p.join("include").join("nvsdk_ngx_vk.h").exists()) else {
        println!("cargo:warning=DLSS_SDK not set (or not the DLSS SDK root): building without DLSS Ray Reconstruction");
        return;
    };
    let vk_include = env::var("VULKAN_SDK").map(|s| PathBuf::from(s).join("Include")).expect("VULKAN_SDK is needed for the DLSS shim");
    cc::Build::new()
        .cpp(true)
        .file("native/dlss_shim.cpp")
        .include(sdk.join("include"))
        .include(vk_include)
        .flag_if_supported("/std:c++17")
        .warnings(false)
        .compile("akari_dlss_shim");
    println!("cargo:rustc-link-search=native={}", sdk.join("lib").join("Windows_x86_64").join("x64").display());
    println!("cargo:rustc-link-lib=static=nvsdk_ngx_d");
    println!("cargo:rustc-cfg=dlss");
}
