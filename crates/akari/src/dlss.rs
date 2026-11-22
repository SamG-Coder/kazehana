//! NVIDIA DLSS Ray Reconstruction (denoiser + upscaler) through the NGX shim in
//! `native/dlss_shim.cpp`. Compiled in only when build.rs found the DLSS SDK (`cfg(dlss)`);
//! otherwise every entry point reports "not available" and the renderer keeps TAA.

use ash::vk;
use std::ffi::CString;

/// Quality modes and their render-resolution scale (per axis).
pub const MODES: [(&str, f32); 5] = [("DLAA", 1.0), ("Quality", 0.667), ("Balanced", 0.58), ("Performance", 0.5), ("Ultra Performance", 0.333)];

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SiImage {
    pub image: vk::Image,
    pub view: vk::ImageView,
    pub format: vk::Format,
    pub width: u32,
    pub height: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SiDlssEval {
    pub color: SiImage,
    pub output: SiImage,
    pub depth: SiImage,
    pub motion: SiImage,
    pub normal_rough: SiImage,
    pub diff_albedo: SiImage,
    pub spec_albedo: SiImage,
    pub jitter_x: f32,
    pub jitter_y: f32,
    pub render_w: u32,
    pub render_h: u32,
    pub reset: i32,
    pub world_to_view: [f32; 16],
    pub view_to_clip: [f32; 16],
    pub frame_time_ms: f32,
}

#[cfg(dlss)]
mod ffi {
    use super::SiDlssEval;
    use ash::vk;
    use std::os::raw::c_char;
    extern "C" {
        pub fn si_dlss_required_extensions(ic: *mut u32, ie: *mut *const *const c_char, dc: *mut u32, de: *mut *const *const c_char) -> i32;
        pub fn si_dlss_init(
            instance: vk::Instance,
            pd: vk::PhysicalDevice,
            device: vk::Device,
            gipa: vk::PFN_vkGetInstanceProcAddr,
            gdpa: vk::PFN_vkGetDeviceProcAddr,
            dll_dir: *const u16,
            data_dir: *const u16,
        ) -> i32;
        pub fn si_dlss_create(cb: vk::CommandBuffer, rw: u32, rh: u32, ow: u32, oh: u32, quality: i32) -> i32;
        pub fn si_dlss_evaluate(cb: vk::CommandBuffer, e: *const SiDlssEval) -> i32;
        pub fn si_dlss_shutdown();
    }
}

/// Instance and device extensions NGX needs (empty without DLSS).
pub fn required_extensions() -> (Vec<CString>, Vec<CString>) {
    #[cfg(dlss)]
    unsafe {
        let (mut ic, mut dc) = (0u32, 0u32);
        let (mut ie, mut de) = (std::ptr::null(), std::ptr::null());
        if ffi::si_dlss_required_extensions(&mut ic, &mut ie, &mut dc, &mut de) == 0 {
            let list = |n: u32, p: *const *const std::os::raw::c_char| -> Vec<CString> {
                (0..n as usize).map(|i| std::ffi::CStr::from_ptr(*p.add(i)).to_owned()).collect()
            };
            return (list(ic, ie), list(dc, de));
        }
    }
    (Vec::new(), Vec::new())
}

fn wide(s: &std::path::Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    s.as_os_str().encode_wide().chain(std::iter::once(0)).collect()
}

/// Initialise NGX; Ok(()) when Ray Reconstruction can be used, Err(reason) otherwise.
#[allow(unused_variables)]
pub fn init(entry: &ash::Entry, instance: &ash::Instance, pd: vk::PhysicalDevice, device: &ash::Device) -> Result<(), String> {
    #[cfg(dlss)]
    unsafe {
        // nvngx_dlssd.dll: in AKARI_DLSS_DIR, or in assets/dlss.
        let dll_dir = std::env::var("AKARI_DLSS_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default().join("assets").join("dlss"));
        let data_dir = std::env::temp_dir().join("akari_ngx");
        let _ = std::fs::create_dir_all(&data_dir);
        let (dw, tw) = (wide(&dll_dir), wide(&data_dir));
        let r = ffi::si_dlss_init(
            instance.handle(),
            pd,
            device.handle(),
            entry.static_fn().get_instance_proc_addr,
            instance.fp_v1_0().get_device_proc_addr,
            dw.as_ptr(),
            tw.as_ptr(),
        );
        return match r {
            0 => Ok(()),
            -2 => Err("Ray Reconstruction not available (driver or nvngx_dlssd.dll missing?)".into()),
            e => Err(format!("NGX init failed: 0x{:08X} (nvngx_dlssd.dll searched in {})", e as u32, dll_dir.display())),
        };
    }
    #[allow(unreachable_code)]
    Err("built without the DLSS SDK (set DLSS_SDK)".into())
}

#[allow(unused_variables)]
pub fn create(cb: vk::CommandBuffer, render: (u32, u32), output: (u32, u32), quality: usize) -> Result<(), String> {
    #[cfg(dlss)]
    unsafe {
        let r = ffi::si_dlss_create(cb, render.0, render.1, output.0, output.1, quality as i32);
        return if r == 0 { Ok(()) } else { Err(format!("DLSS-RR create failed: 0x{:08X}", r as u32)) };
    }
    #[allow(unreachable_code)]
    Err("no DLSS".into())
}

#[allow(unused_variables)]
pub fn evaluate(cb: vk::CommandBuffer, e: &SiDlssEval) -> Result<(), String> {
    #[cfg(dlss)]
    unsafe {
        let r = ffi::si_dlss_evaluate(cb, e);
        return if r == 0 { Ok(()) } else { Err(format!("DLSS-RR evaluate failed: 0x{:08X}", r as u32)) };
    }
    #[allow(unreachable_code)]
    Err("no DLSS".into())
}

/// The renderer's sub-pixel jitter (globals.glsl taaJitter): Halton(2,3), 32-frame cycle, [0,1)^2.
pub fn jitter(frame: u32) -> (f32, f32) {
    let halton = |mut i: u32, b: u32| {
        let (mut f, mut r) = (1.0f32, 0.0f32);
        while i > 0 {
            f /= b as f32;
            r += f * (i % b) as f32;
            i /= b;
        }
        r
    };
    let i = (frame % 32) + 1;
    (halton(i, 2), halton(i, 3))
}

/// Release the Ray Reconstruction feature and NGX (before the device is destroyed).
pub fn shutdown() {
    #[cfg(dlss)]
    unsafe {
        ffi::si_dlss_shutdown();
    }
}
