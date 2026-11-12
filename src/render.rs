use crate::assets;
use crate::model::{hex_to_rgba, Style};
use std::path::{Path, PathBuf};
use tiny_skia::{FilterQuality, Pixmap, PixmapPaint, Transform};

#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    Svg(String),
    File(PathBuf),
}

impl Source {
    pub fn from_config(value: &str) -> Self {
        if let Some(name) = value.strip_prefix("builtin:") {
            if let Some(svg) = assets::builtin_svg(name) {
                return Self::Svg(svg.to_string());
            }
        }
        if let Some(path) = value.strip_prefix("file:") {
            return Self::File(PathBuf::from(path));
        }
        Self::Svg(
            assets::builtin_svg("cross-dot")
                .expect("default crosshair is embedded")
                .to_string(),
        )
    }

    pub fn label(&self) -> String {
        match self {
            Self::Svg(_) => "Built-in crosshair".to_string(),
            Self::File(path) => path
                .file_name()
                .and_then(|n| n.to_str())
                .map(assets::pretty_name)
                .unwrap_or_else(|| "Custom crosshair".to_string()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RenderOptions {
    pub size: u32,
    pub hue: f32,
    pub rotation: f32,
    pub opacity: f32,
    pub fill: Option<[u8; 4]>,
    pub outline: bool,
    pub outline_width: u32,
    pub outline_color: [u8; 4],
    pub glow: bool,
    pub glow_color: [u8; 4],
}

impl RenderOptions {
    pub fn from_style(style: &Style, size: u32) -> Self {
        Self {
            size: size.clamp(1, 1024),
            hue: style.hue,
            rotation: style.rotation,
            opacity: style.clamped_opacity(),
            fill: style
                .fill_color
                .as_deref()
                .map(|c| hex_to_rgba(Some(c), [255, 255, 255, 255])),
            outline: style.outline,
            outline_width: style.outline_width.clamp(1, 5),
            outline_color: hex_to_rgba(Some(&style.outline_color), [0, 0, 0, 255]),
            glow: style.glow,
            glow_color: hex_to_rgba(Some(&style.glow_color), [108, 140, 255, 255]),
        }
    }

    pub fn thumbnail(_style: &Style, size: u32) -> Self {
        Self {
            size,
            hue: 0.0,
            rotation: 0.0,
            opacity: 1.0,
            fill: None,
            outline: false,
            outline_width: 1,
            outline_color: [0, 0, 0, 255],
            glow: false,
            glow_color: [108, 140, 255, 255],
        }
    }
}

/// Render a crosshair into a premultiplied RGBA pixmap.
///
/// The pipeline mirrors the CSS effects of the original app: hue rotation
/// (unless a solid fill is used), stacked outline shadows, a soft glow, then
/// rotation and opacity.
pub fn render(source: &Source, opts: &RenderOptions) -> Pixmap {
    let size = opts.size.max(1);
    let Some(mut main) = load_source(source, size) else {
        return Pixmap::new(size, size).expect("valid pixmap size");
    };

    if let Some(fill) = opts.fill {
        apply_fill(&mut main, fill);
    } else if opts.hue.abs() > 0.01 {
        apply_hue(&mut main, opts.hue);
    }

    let mut composed = Pixmap::new(size, size).expect("valid pixmap size");

    if opts.glow {
        let radius = (6.0 + opts.outline_width as f32 * 3.0).round() as usize;
        let glow = glow_layer(&main, opts.glow_color, radius);
        if let Some(glow) = glow {
            composed.draw_pixmap(
                0,
                0,
                glow.as_ref(),
                &PixmapPaint::default(),
                Transform::identity(),
                None,
            );
        }
    }

    if opts.outline {
        let tint = tinted(&main, opts.outline_color);
        let width = opts.outline_width.clamp(1, 5) as i32;
        for i in 1..=width {
            for (dx, dy) in [(i, 0), (-i, 0), (0, i), (0, -i)] {
                composed.draw_pixmap(
                    dx,
                    dy,
                    tint.as_ref(),
                    &PixmapPaint::default(),
                    Transform::identity(),
                    None,
                );
            }
        }
    }

    composed.draw_pixmap(
        0,
        0,
        main.as_ref(),
        &PixmapPaint::default(),
        Transform::identity(),
        None,
    );

    if opts.rotation.abs() > 0.01 {
        if let Some(rotated) = rotate(&composed, opts.rotation) {
            composed = rotated;
        }
    }

    if opts.opacity < 0.999 {
        apply_opacity(&mut composed, opts.opacity);
    }

    composed
}

/// Render a source image and fit it into a square, preserving aspect ratio.
pub fn load_source(source: &Source, size: u32) -> Option<Pixmap> {
    match source {
        Source::Svg(svg) => render_svg(svg, size),
        Source::File(path) => load_file(path, size),
    }
}

fn load_file(path: &Path, size: u32) -> Option<Pixmap> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "svg" => {
            let svg = std::fs::read_to_string(path).ok()?;
            render_svg(&svg, size)
        }
        "png" => {
            let data = std::fs::read(path).ok()?;
            let src = Pixmap::decode_png(&data).ok()?;
            fit_pixmap(&src, size)
        }
        _ => None,
    }
}

fn render_svg(svg: &str, size: u32) -> Option<Pixmap> {
    let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default()).ok()?;
    let mut pixmap = Pixmap::new(size, size)?;
    let w = tree.size().width();
    let h = tree.size().height();
    if !(w > 0.0 && h > 0.0) {
        return None;
    }
    let scale = (size as f32 / w).min(size as f32 / h);
    let tx = (size as f32 - w * scale) * 0.5;
    let ty = (size as f32 - h * scale) * 0.5;
    let ts = Transform::from_scale(scale, scale).post_translate(tx, ty);
    resvg::render(&tree, ts, &mut pixmap.as_mut());
    Some(pixmap)
}

fn fit_pixmap(src: &Pixmap, size: u32) -> Option<Pixmap> {
    let mut out = Pixmap::new(size, size)?;
    let scale = (size as f32 / src.width() as f32).min(size as f32 / src.height() as f32);
    let tx = (size as f32 - src.width() as f32 * scale) * 0.5;
    let ty = (size as f32 - src.height() as f32 * scale) * 0.5;
    let ts = Transform::from_scale(scale, scale).post_translate(tx, ty);
    out.draw_pixmap(
        0,
        0,
        src.as_ref(),
        &PixmapPaint {
            quality: FilterQuality::Bicubic,
            ..Default::default()
        },
        ts,
        None,
    );
    Some(out)
}

fn tinted(src: &Pixmap, color: [u8; 4]) -> Pixmap {
    let mut out = Pixmap::new(src.width(), src.height()).expect("matching pixmap size");
    let data = out.data_mut();
    for (dst, px) in data.chunks_exact_mut(4).zip(src.data().chunks_exact(4)) {
        let a = px[3];
        dst[0] = (color[0] as u16 * a as u16 / 255) as u8;
        dst[1] = (color[1] as u16 * a as u16 / 255) as u8;
        dst[2] = (color[2] as u16 * a as u16 / 255) as u8;
        dst[3] = a;
    }
    out
}

fn glow_layer(src: &Pixmap, color: [u8; 4], radius: usize) -> Option<Pixmap> {
    let w = src.width() as usize;
    let h = src.height() as usize;
    let mut alpha: Vec<u8> = src.data().chunks_exact(4).map(|p| p[3]).collect();
    box_blur_alpha(&mut alpha, w, h, radius.max(1), 3);

    let mut out = Pixmap::new(w as u32, h as u32)?;
    let data = out.data_mut();
    for (i, a) in alpha.into_iter().enumerate() {
        let a = (a as u16 * color[3] as u16 / 255) as u8;
        let o = i * 4;
        data[o] = (color[0] as u16 * a as u16 / 255) as u8;
        data[o + 1] = (color[1] as u16 * a as u16 / 255) as u8;
        data[o + 2] = (color[2] as u16 * a as u16 / 255) as u8;
        data[o + 3] = a;
    }
    Some(out)
}

fn box_blur_alpha(alpha: &mut [u8], w: usize, h: usize, radius: usize, passes: usize) {
    if w == 0 || h == 0 {
        return;
    }
    let mut tmp = vec![0u8; alpha.len()];
    let diameter = radius * 2 + 1;
    for _ in 0..passes {
        for y in 0..h {
            let row = &alpha[y * w..(y + 1) * w];
            let out_row = &mut tmp[y * w..(y + 1) * w];
            let mut sum: u32 = 0;
            for i in 0..=radius.min(w - 1) {
                sum += row[i] as u32;
            }
            for x in 0..w {
                if x + radius < w {
                    sum += row[x + radius] as u32;
                }
                if x > radius {
                    sum -= row[x - radius - 1] as u32;
                }
                out_row[x] = (sum / diameter as u32).min(255) as u8;
            }
        }
        for x in 0..w {
            let mut sum: u32 = 0;
            for y in 0..=radius.min(h - 1) {
                sum += tmp[y * w + x] as u32;
            }
            for y in 0..h {
                if y + radius < h {
                    sum += tmp[(y + radius) * w + x] as u32;
                }
                if y > radius {
                    sum -= tmp[(y - radius - 1) * w + x] as u32;
                }
                alpha[y * w + x] = (sum / diameter as u32).min(255) as u8;
            }
        }
    }
}

fn apply_fill(pixmap: &mut Pixmap, color: [u8; 4]) {
    for px in pixmap.data_mut().chunks_exact_mut(4) {
        let a = px[3];
        px[0] = (color[0] as u16 * a as u16 / 255) as u8;
        px[1] = (color[1] as u16 * a as u16 / 255) as u8;
        px[2] = (color[2] as u16 * a as u16 / 255) as u8;
    }
}

fn apply_hue(pixmap: &mut Pixmap, degrees: f32) {
    let angle = degrees.to_radians();
    let (sin, cos) = angle.sin_cos();
    // CSS filter: hue-rotate() matrix.
    let m = [
        0.213 + cos * 0.787 - sin * 0.213,
        0.715 - cos * 0.715 - sin * 0.715,
        0.072 - cos * 0.072 + sin * 0.928,
        0.213 - cos * 0.213 + sin * 0.143,
        0.715 + cos * 0.285 + sin * 0.140,
        0.072 - cos * 0.072 - sin * 0.283,
        0.213 - cos * 0.213 - sin * 0.787,
        0.715 - cos * 0.715 + sin * 0.715,
        0.072 + cos * 0.928 + sin * 0.072,
    ];
    for px in pixmap.data_mut().chunks_exact_mut(4) {
        let a = px[3];
        if a == 0 {
            continue;
        }
        let af = a as f32;
        let r = px[0] as f32 * 255.0 / af;
        let g = px[1] as f32 * 255.0 / af;
        let b = px[2] as f32 * 255.0 / af;
        let nr = (m[0] * r + m[1] * g + m[2] * b).clamp(0.0, 255.0);
        let ng = (m[3] * r + m[4] * g + m[5] * b).clamp(0.0, 255.0);
        let nb = (m[6] * r + m[7] * g + m[8] * b).clamp(0.0, 255.0);
        px[0] = (nr * af / 255.0).round() as u8;
        px[1] = (ng * af / 255.0).round() as u8;
        px[2] = (nb * af / 255.0).round() as u8;
    }
}

fn apply_opacity(pixmap: &mut Pixmap, opacity: f32) {
    let o = opacity.clamp(0.0, 1.0);
    for px in pixmap.data_mut().chunks_exact_mut(4) {
        for c in px.iter_mut() {
            *c = (*c as f32 * o).round() as u8;
        }
    }
}

fn rotate(src: &Pixmap, degrees: f32) -> Option<Pixmap> {
    let mut out = Pixmap::new(src.width(), src.height())?;
    let cx = src.width() as f32 * 0.5;
    let cy = src.height() as f32 * 0.5;
    out.draw_pixmap(
        0,
        0,
        src.as_ref(),
        &PixmapPaint {
            quality: FilterQuality::Bicubic,
            ..Default::default()
        },
        Transform::from_rotate_at(degrees, cx, cy),
        None,
    );
    Some(out)
}

/// Convert a premultiplied RGBA pixmap into the byte order expected by
/// X11 ARGB32 and Wayland ARGB8888 on little-endian systems (BGRA).
pub fn pixmap_to_bgra(pixmap: &Pixmap) -> Vec<u8> {
    let mut out = Vec::with_capacity(pixmap.data().len());
    for px in pixmap.data().chunks_exact(4) {
        out.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
    }
    out
}

pub fn pixmap_to_argb_network(pixmap: &Pixmap) -> Vec<u8> {
    let mut out = Vec::with_capacity(pixmap.data().len());
    for px in pixmap.data().chunks_exact(4) {
        out.extend_from_slice(&[px[3], px[0], px[1], px[2]]);
    }
    out
}

/// Create a crisp icon pixmap with the app's crosshair mark.
pub fn brand_icon(size: u32) -> Pixmap {
    let mut pixmap = Pixmap::new(size, size.max(1)).unwrap_or_else(|| {
        Pixmap::new(1, 1).expect("1x1 pixmap")
    });
    if size == 0 {
        return pixmap;
    }
    let s = size as f32;
    let r = s * 0.22;

    // Rounded tile matching the application icon background.
    let mut background = tiny_skia::Paint::default();
    background.set_color_rgba8(29, 28, 42, 255);
    background.anti_alias = true;
    let mut pb = tiny_skia::PathBuilder::new();
    pb.move_to(r, 0.0);
    pb.line_to(s - r, 0.0);
    pb.quad_to(s, 0.0, s, r);
    pb.line_to(s, s - r);
    pb.quad_to(s, s, s - r, s);
    pb.line_to(r, s);
    pb.quad_to(0.0, s, 0.0, s - r);
    pb.line_to(0.0, r);
    pb.quad_to(0.0, 0.0, r, 0.0);
    pb.close();
    let Some(tile) = pb.finish() else {
        return pixmap;
    };
    pixmap.fill_path(
        &tile,
        &background,
        tiny_skia::FillRule::Winding,
        Transform::identity(),
        None,
    );

    let mut edge = tiny_skia::Paint::default();
    edge.set_color_rgba8(58, 55, 82, 255);
    edge.anti_alias = true;
    pixmap.stroke_path(
        &tile,
        &edge,
        &tiny_skia::Stroke {
            width: (s * 0.015).max(1.0),
            ..Default::default()
        },
        Transform::identity(),
        None,
    );

    let mut accent = tiny_skia::Paint::default();
    accent.set_color_rgba8(183, 174, 228, 255);
    accent.anti_alias = true;
    let center = s * 0.5;
    let arm = s * 0.30;
    let gap = s * 0.05;
    let stroke = (s * 0.045).max(1.2);
    for (x1, y1, x2, y2) in [
        (center, center - gap, center, center - arm),
        (center, center + gap, center, center + arm),
        (center - gap, center, center - arm, center),
        (center + gap, center, center + arm, center),
    ] {
        let mut pb = tiny_skia::PathBuilder::new();
        pb.move_to(x1, y1);
        pb.line_to(x2, y2);
        if let Some(path) = pb.finish() {
            pixmap.stroke_path(
                &path,
                &accent,
                &tiny_skia::Stroke {
                    width: stroke,
                    line_cap: tiny_skia::LineCap::Round,
                    ..Default::default()
                },
                Transform::identity(),
                None,
            );
        }
    }
    let mut ring = tiny_skia::PathBuilder::new();
    ring.push_circle(center, center, s * 0.14);
    if let Some(path) = ring.finish() {
        pixmap.stroke_path(
            &path,
            &accent,
            &tiny_skia::Stroke {
                width: (s * 0.04).max(1.0),
                ..Default::default()
            },
            Transform::identity(),
            None,
        );
    }
    pixmap
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Style;

    #[test]
    fn renders_all_builtins() {
        let style = Style::default();
        for (name, svg) in assets::BUILTINS {
            let pixmap = render(&Source::Svg((*svg).to_string()), &RenderOptions::from_style(&style, 64));
            let opaque = pixmap.data().chunks_exact(4).filter(|p| p[3] > 0).count();
            assert!(opaque > 0, "crosshair {name} rendered nothing");
        }
    }

    #[test]
    fn effects_produce_pixels_outside_the_original_shape() {
        let mut style = Style::default();
        style.crosshair = "builtin:dot".to_string();
        style.outline = true;
        style.outline_width = 3;
        style.outline_color = "#ff0000".to_string();
        let pl = render(
            &Source::from_config(&style.crosshair),
            &RenderOptions::from_style(&style, 64),
        );
        let red = pl
            .data()
            .chunks_exact(4)
            .filter(|p| p[3] > 0 && p[0] > 100 && p[1] < 80)
            .count();
        assert!(red > 0, "outline did not produce red pixels");
    }

    #[test]
    fn designer_svg_is_valid() {
        let d = crate::model::DesignerDesign::default();
        let svg = d.to_svg();
        let tree = resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default());
        assert!(tree.is_ok());
        let pl = render(&Source::Svg(svg), &RenderOptions::from_style(&Style::default(), 64));
        assert!(pl.data().chunks_exact(4).any(|p| p[3] > 0));
    }

    #[test]
    fn png_roundtrip() {
        let p = brand_icon(64);
        let data = p.encode_png().unwrap();
        let decoded = Pixmap::decode_png(&data).unwrap();
        assert_eq!(decoded.width(), 64);
    }
}
