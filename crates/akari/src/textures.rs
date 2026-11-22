//! Material textures: decode (PNG), build mips on the CPU, block-compress (BC7 colour, BC5 normal
//! maps, BC7 for packed data such as ORM) and cache the result in assets/cache/tex, keyed by the
//! source file. Adapted from SmartImposter's textures.rs; the encoder is its bc7.rs.

use crate::gpu::{color_range, Gpu, Image};
use anyhow::{bail, Context, Result};
use ash::vk;
use gpu_allocator::MemoryLocation;
use std::path::{Path, PathBuf};

/// What a texture holds, which decides its format and how its mips are averaged.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TexKind {
    /// sRGB colour (+ alpha): BC7 sRGB.
    Color,
    /// Tangent-space normal map: x, y in BC5 (z is rebuilt in the shader).
    Normal,
    /// Linear data in up to four channels (glTF metallic-roughness / ORM): BC7 unorm.
    Data,
    /// One exact byte per texel (maps of codes read with texelFetch): R8 unorm, full size, no mips,
    /// not compressed or cached.
    Raw8,
    /// A page drawn while the game runs (the calendar's month, the journal's spread): sRGB RGBA8,
    /// uncompressed with simple mips, not cached, so it can be made again quickly whenever what's
    /// on it changes.
    Page,
}

impl TexKind {
    fn srgb(self) -> bool {
        matches!(self, TexKind::Color | TexKind::Page)
    }
    fn format(self) -> vk::Format {
        match self {
            TexKind::Color => vk::Format::BC7_SRGB_BLOCK,
            TexKind::Normal => vk::Format::BC5_UNORM_BLOCK,
            TexKind::Data => vk::Format::BC7_UNORM_BLOCK,
            TexKind::Raw8 => vk::Format::R8_UNORM,
            TexKind::Page => vk::Format::R8G8B8A8_SRGB,
        }
    }
}

const CACHE_DIR: &str = "assets/cache/tex";
const CACHE_VERSION: u32 = 1;

struct Encoded {
    w: u32,
    h: u32,
    /// encoded blocks of every mip level, largest first
    levels: Vec<Vec<u8>>,
}

pub fn decode_png(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>)> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = dec.read_info()?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf)?;
    let px = &buf[..info.buffer_size()];
    let rgba = match info.color_type {
        png::ColorType::Rgba => px.to_vec(),
        png::ColorType::Rgb => px.chunks(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => px.chunks(2).flat_map(|c| [c[0], c[0], c[0], c[1]]).collect(),
        png::ColorType::Grayscale => px.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        other => bail!("unsupported PNG colour type {other:?}"),
    };
    Ok((info.width, info.height, rgba))
}

/// Halve with a 2x2 box filter until it fits in `max` texels per side.
fn shrink(mut w: u32, mut h: u32, mut px: Vec<u8>, max: u32, srgb: bool) -> (u32, u32, Vec<u8>) {
    while w.max(h) > max && w >= 2 && h >= 2 {
        let chain = mip_chain_levels(w, h, &px, srgb, 1);
        let (nw, nh, p) = chain.into_iter().nth(1).unwrap();
        (w, h, px) = (nw, nh, p);
    }
    (w, h, px)
}

fn srgb_tables() -> (Vec<f32>, impl Fn(f32) -> u8) {
    let to_lin: Vec<f32> = (0..256)
        .map(|c| {
            let c = c as f32 / 255.0;
            if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
        })
        .collect();
    let to_8 = |v: f32| -> u8 {
        let v = v.clamp(0.0, 1.0);
        let e = if v <= 0.0031308 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 };
        (e * 255.0 + 0.5) as u8
    };
    (to_lin, to_8)
}

/// Up to `count` further mip levels after the source (2x2 box; sRGB colour averaged in linear light).
fn mip_chain_levels(w: u32, h: u32, px: &[u8], srgb: bool, count: usize) -> Vec<(u32, u32, Vec<u8>)> {
    let (to_lin, to_8) = srgb_tables();
    let mut out = vec![(w, h, px.to_vec())];
    let (mut cw, mut ch) = (w, h);
    while (cw > 1 || ch > 1) && out.len() <= count {
        let (nw, nh) = ((cw / 2).max(1), (ch / 2).max(1));
        let src = &out.last().unwrap().2;
        let mut dst = vec![0u8; (nw * nh * 4) as usize];
        for y in 0..nh {
            for x in 0..nw {
                let xs = [(2 * x).min(cw - 1), (2 * x + 1).min(cw - 1)];
                let ys = [(2 * y).min(ch - 1), (2 * y + 1).min(ch - 1)];
                for c in 0..4 {
                    let mut sum = 0.0;
                    for yy in ys {
                        for xx in xs {
                            let v = src[((yy * cw + xx) * 4 + c) as usize];
                            sum += if c == 3 || !srgb { v as f32 / 255.0 } else { to_lin[v as usize] };
                        }
                    }
                    let a = sum / 4.0;
                    dst[((y * nw + x) * 4 + c) as usize] = if c == 3 || !srgb { (a * 255.0 + 0.5) as u8 } else { to_8(a) };
                }
            }
        }
        out.push((nw, nh, dst));
        (cw, ch) = (nw, nh);
    }
    out
}

/// Normal maps: renormalise every mip (averaging shortens normals).
fn renormalise(px: &mut [u8]) {
    for p in px.chunks_mut(4) {
        let v = [p[0] as f32 / 127.5 - 1.0, p[1] as f32 / 127.5 - 1.0, p[2] as f32 / 127.5 - 1.0];
        let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-4);
        for i in 0..3 {
            p[i] = ((v[i] / l * 0.5 + 0.5) * 255.0 + 0.5) as u8;
        }
    }
}

fn encode(w: u32, h: u32, px: Vec<u8>, kind: TexKind) -> Encoded {
    let mut chain = mip_chain_levels(w, h, &px, kind.srgb(), usize::MAX);
    if kind == TexKind::Normal {
        for (_, _, p) in chain.iter_mut().skip(1) {
            renormalise(p);
        }
    }
    let levels = chain
        .iter()
        .map(|(mw, mh, p)| match kind {
            TexKind::Normal => crate::bc7::encode_channels(*mw, *mh, p, &[0, 1]),
            _ => crate::bc7::encode_image(*mw, *mh, p),
        })
        .collect();
    Encoded { w, h, levels }
}

fn level_bytes(w: u32, h: u32) -> usize {
    (w.div_ceil(4) * h.div_ceil(4)) as usize * 16
}

fn cache_path(key: &str, max: u32, kind: TexKind) -> PathBuf {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (key, max, kind, CACHE_VERSION).hash(&mut h);
    Path::new(CACHE_DIR).join(format!("{:016x}.bc", h.finish()))
}

/// A cache key for a file: path, size and modification time.
pub fn file_key(path: &Path) -> String {
    let meta = std::fs::metadata(path).ok();
    let len = meta.as_ref().map_or(0, |m| m.len());
    let mtime = meta.and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs());
    format!("{}|{len}|{mtime}", path.display())
}

fn read_cache(path: &Path) -> Option<Encoded> {
    let b = std::fs::read(path).ok()?;
    if b.len() < 20 || &b[..8] != b"AKARIBC1" {
        return None;
    }
    let u = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    let (w, h, mips) = (u(8), u(12), u(16));
    let mut levels = Vec::new();
    let mut o = 20;
    for m in 0..mips {
        let n = level_bytes((w >> m).max(1), (h >> m).max(1));
        levels.push(b.get(o..o + n)?.to_vec());
        o += n;
    }
    Some(Encoded { w, h, levels })
}

fn write_cache(path: &Path, t: &Encoded) {
    let mut out = Vec::new();
    out.extend_from_slice(b"AKARIBC1");
    for v in [t.w, t.h, t.levels.len() as u32] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for l in &t.levels {
        out.extend_from_slice(l);
    }
    let _ = std::fs::create_dir_all(CACHE_DIR);
    let tmp = path.with_extension("part");
    if std::fs::write(&tmp, out).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
}

fn upload(gpu: &Gpu, t: &Encoded, format: vk::Format, name: &str) -> Result<Image> {
    let mips = t.levels.len() as u32;
    let img = gpu.create_image(t.w, t.h, mips, format, vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST, name)?;
    let total: usize = t.levels.iter().map(|l| l.len()).sum();
    let mut staging = gpu.create_buffer(total as u64, vk::BufferUsageFlags::TRANSFER_SRC, MemoryLocation::CpuToGpu, "tex staging")?;
    let mut regions = Vec::new();
    let mut o = 0usize;
    for (m, l) in t.levels.iter().enumerate() {
        staging.mapped()[o..o + l.len()].copy_from_slice(l);
        regions.push(vk::BufferImageCopy {
            buffer_offset: o as u64,
            image_subresource: vk::ImageSubresourceLayers { aspect_mask: vk::ImageAspectFlags::COLOR, mip_level: m as u32, base_array_layer: 0, layer_count: 1 },
            image_extent: vk::Extent3D { width: (t.w >> m).max(1), height: (t.h >> m).max(1), depth: 1 },
            ..Default::default()
        });
        o += l.len();
    }
    let raw = img.raw;
    gpu.one_time(|cb| {
        gpu.image_barrier(cb, raw, vk::ImageLayout::UNDEFINED, vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::PipelineStageFlags2::NONE, vk::AccessFlags2::NONE, vk::PipelineStageFlags2::TRANSFER, vk::AccessFlags2::TRANSFER_WRITE);
        unsafe { gpu.device.cmd_copy_buffer_to_image(cb, staging.raw, raw, vk::ImageLayout::TRANSFER_DST_OPTIMAL, &regions) };
        gpu.image_barrier(cb, raw, vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL, vk::PipelineStageFlags2::TRANSFER, vk::AccessFlags2::TRANSFER_WRITE, vk::PipelineStageFlags2::ALL_COMMANDS, vk::AccessFlags2::SHADER_READ);
    })?;
    gpu.destroy_buffer(&mut staging);
    Ok(img)
}

/// Upload already block-compressed levels (largest first) as a sampled texture.
pub fn upload_levels(gpu: &Gpu, w: u32, h: u32, levels: &[Vec<u8>], format: vk::Format, name: &str) -> Result<Image> {
    upload(gpu, &Encoded { w, h, levels: levels.to_vec() }, format, name)
}

/// An uncompressed 1x1 texture (defaults for missing maps).
pub fn solid(gpu: &Gpu, rgba: [u8; 4], srgb: bool, name: &str) -> Result<Image> {
    let format = if srgb { vk::Format::R8G8B8A8_SRGB } else { vk::Format::R8G8B8A8_UNORM };
    let img = gpu.create_image(1, 1, 1, format, vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST, name)?;
    let mut staging = gpu.create_buffer(4, vk::BufferUsageFlags::TRANSFER_SRC, MemoryLocation::CpuToGpu, "tex staging")?;
    staging.mapped()[..4].copy_from_slice(&rgba);
    let raw = img.raw;
    gpu.one_time(|cb| {
        gpu.image_barrier(cb, raw, vk::ImageLayout::UNDEFINED, vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::PipelineStageFlags2::NONE, vk::AccessFlags2::NONE, vk::PipelineStageFlags2::TRANSFER, vk::AccessFlags2::TRANSFER_WRITE);
        let region = vk::BufferImageCopy {
            image_subresource: vk::ImageSubresourceLayers { aspect_mask: vk::ImageAspectFlags::COLOR, mip_level: 0, base_array_layer: 0, layer_count: 1 },
            image_extent: vk::Extent3D { width: 1, height: 1, depth: 1 },
            ..Default::default()
        };
        unsafe { gpu.device.cmd_copy_buffer_to_image(cb, staging.raw, raw, vk::ImageLayout::TRANSFER_DST_OPTIMAL, &[region]) };
        gpu.image_barrier(cb, raw, vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL, vk::PipelineStageFlags2::TRANSFER, vk::AccessFlags2::TRANSFER_WRITE, vk::PipelineStageFlags2::ALL_COMMANDS, vk::AccessFlags2::SHADER_READ);
    })?;
    gpu.destroy_buffer(&mut staging);
    let _ = color_range(0, 1);
    Ok(img)
}

/// One texture to load. `bytes` loads lazily (only when the cache misses).
pub struct TexJob {
    pub key: String,
    pub name: String,
    pub kind: TexKind,
    pub source: TexSource,
}

pub enum TexSource {
    File(PathBuf),
    Bytes(Vec<u8>),
    /// Pixels already in memory: width, height, sRGB RGBA8.
    Rgba(u32, u32, Vec<u8>),
}

/// Largest texture side kept (env AKARI_TEX_MAX, default 2048).
pub fn tex_max() -> u32 {
    std::env::var("AKARI_TEX_MAX").ok().and_then(|v| v.parse().ok()).unwrap_or(2048)
}

/// Load textures on all cores (cache, or decode + encode + cache), then upload them in order.
pub fn load(gpu: &Gpu, jobs: &[TexJob]) -> Result<Vec<Image>> {
    let t0 = std::time::Instant::now();
    let max = tex_max();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let encoded_count = std::sync::atomic::AtomicUsize::new(0);
    let done: Vec<std::sync::Mutex<Option<Result<Encoded>>>> = jobs.iter().map(|_| std::sync::Mutex::new(None)).collect();
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if i >= jobs.len() {
                    break;
                }
                let job = &jobs[i];
                if job.kind == TexKind::Page {
                    let r = (|| -> Result<Encoded> {
                        let (w, h, px) = match &job.source {
                            TexSource::Rgba(w, h, px) => (*w, *h, px.clone()),
                            TexSource::File(p) => decode_png(&std::fs::read(p).with_context(|| format!("reading {}", p.display()))?)?,
                            TexSource::Bytes(b) => decode_png(b)?,
                        };
                        let chain = mip_chain_levels(w, h, &px, true, usize::MAX);
                        Ok(Encoded { w, h, levels: chain.into_iter().map(|(_, _, p)| p).collect() })
                    })();
                    *done[i].lock().unwrap() = Some(r);
                    continue;
                }
                if job.kind == TexKind::Raw8 {
                    let r = (|| -> Result<Encoded> {
                        let bytes = match &job.source {
                            TexSource::File(p) => std::fs::read(p).with_context(|| format!("reading {}", p.display()))?,
                            TexSource::Bytes(b) => b.clone(),
                            TexSource::Rgba(..) => anyhow::bail!("{}: raw pixels are for pages", job.name),
                        };
                        let (w, h, px) = decode_png(&bytes).with_context(|| format!("decoding {}", job.name))?;
                        Ok(Encoded { w, h, levels: vec![px.chunks(4).map(|c| c[0]).collect()] })
                    })();
                    *done[i].lock().unwrap() = Some(r);
                    continue;
                }
                let cache = cache_path(&job.key, max, job.kind);
                if let Some(c) = read_cache(&cache) {
                    *done[i].lock().unwrap() = Some(Ok(c));
                    continue;
                }
                let r = (|| -> Result<Encoded> {
                    let (w, h, px) = match &job.source {
                        TexSource::File(p) => decode_png(&std::fs::read(p).with_context(|| format!("reading {}", p.display()))?).with_context(|| format!("decoding {}", job.name))?,
                        TexSource::Bytes(b) => decode_png(b).with_context(|| format!("decoding {}", job.name))?,
                        TexSource::Rgba(w, h, px) => (*w, *h, px.clone()),
                    };
                    let (w, h, px) = shrink(w, h, px, max, job.kind.srgb());
                    let e = encode(w, h, px, job.kind);
                    write_cache(&cache, &e);
                    encoded_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    Ok(e)
                })();
                *done[i].lock().unwrap() = Some(r);
            });
        }
    });
    let mut out = Vec::with_capacity(jobs.len());
    let mut bytes = 0u64;
    for (job, slot) in jobs.iter().zip(done) {
        let tex = slot.into_inner().unwrap().unwrap()?;
        bytes += tex.levels.iter().map(|l| l.len() as u64).sum::<u64>();
        out.push(upload(gpu, &tex, job.kind.format(), &job.name)?);
    }
    if !jobs.is_empty() {
        println!(
            "Textures: {} ({} newly encoded), {:.0} MB on GPU, {:.2}s",
            jobs.len(),
            encoded_count.into_inner(),
            bytes as f64 / 1e6,
            t0.elapsed().as_secs_f32()
        );
    }
    Ok(out)
}
