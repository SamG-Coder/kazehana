//! The controller button drawings (tools/buttons/svg, from make_button_svgs.mjs) as images the
//! game's UI draws: assets/ui/buttons/<name>.png, 96 px high (the drawing's 36 units: 32 and a
//! margin of 2 round it), straight (not premultiplied) RGBA.
//!
//!     cargo run --release -p buttons

use anyhow::{Context, Result};
use std::path::Path;

const HEIGHT: f32 = 96.0;

fn main() -> Result<()> {
    let src = Path::new("tools/buttons/svg");
    let out = Path::new("assets/ui/buttons");
    std::fs::create_dir_all(out)?;
    let mut opt = resvg::usvg::Options::default();
    opt.fontdb_mut().load_system_fonts();
    let mut n = 0;
    for e in std::fs::read_dir(src).context("tools/buttons/svg: run make_button_svgs.mjs first")? {
        let p = e?.path();
        if p.extension().and_then(|x| x.to_str()) != Some("svg") {
            continue;
        }
        let tree = resvg::usvg::Tree::from_data(&std::fs::read(&p)?, &opt).with_context(|| p.display().to_string())?;
        let size = tree.size();
        let s = HEIGHT / size.height();
        let (w, h) = ((size.width() * s).ceil() as u32, HEIGHT as u32);
        let mut pix = resvg::tiny_skia::Pixmap::new(w, h).context("pixmap")?;
        resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(s, s), &mut pix.as_mut());
        // (save_png writes straight alpha: tiny-skia keeps colour premultiplied and undoes it there)
        pix.save_png(out.join(p.with_extension("png").file_name().unwrap()))?;
        n += 1;
    }
    println!("wrote {n} buttons to {}", out.display());
    Ok(())
}
