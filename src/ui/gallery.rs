//! Crosshair gallery helpers: texture creation, custom designs on disk, import
//! and saving of designer output.

use crate::assets;
use crate::config;
use crate::model::{hex_to_rgba, rgba_to_hex, DesignerDesign, Style};
use crate::render::{self, RenderOptions, Source};
use gtk4::{gdk, glib};
use glib::object::Cast;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tiny_skia::Pixmap;

pub type ThumbCache = RefCell<HashMap<String, gdk::Texture>>;

/// Wrap a premultiplied RGBA pixmap in a `GdkTexture` without copying the
/// pixels twice.
pub fn texture_from_pixmap(pixmap: &Pixmap) -> gdk::Texture {
    let width = pixmap.width() as i32;
    let height = pixmap.height() as i32;
    let stride = (width * 4) as usize;
    let bytes = glib::Bytes::from_owned(pixmap.data().to_vec());
    gdk::MemoryTexture::new(
        width,
        height,
        gdk::MemoryFormat::R8g8b8a8Premultiplied,
        &bytes,
        stride,
    )
    .upcast()
}

/// Render the full styled crosshair for the settings preview.
pub fn preview_texture(style: &Style, size: u32) -> gdk::Texture {
    let pixmap = render::render(
        &Source::from_config(&style.crosshair),
        &RenderOptions::from_style(style, size),
    );
    texture_from_pixmap(&pixmap)
}

/// Render a small gallery thumbnail. Built-in and designer SVGs are white, so
/// they are tinted with the theme text color for contrast.
pub fn thumbnail(
    cache: &ThumbCache,
    source: &str,
    size: u32,
    tint: Option<[u8; 4]>,
) -> gdk::Texture {
    let key = format!(
        "{source}|{size}|{}",
        tint.map(rgba_to_hex).unwrap_or_else(|| "natural".to_string())
    );
    if let Some(texture) = cache.borrow().get(&key) {
        return texture.clone();
    }
    let mut options = RenderOptions::thumbnail(&Style::default(), size);
    options.fill = tint;
    let pixmap = render::render(&Source::from_config(source), &options);
    let texture = texture_from_pixmap(&pixmap);
    cache.borrow_mut().insert(key, texture.clone());
    texture
}

/// Human-readable label for a crosshair source.
pub fn label_for_source(source: &str) -> String {
    if let Some(name) = source.strip_prefix("builtin:") {
        return assets::pretty_name(name);
    }
    if let Some(path) = source.strip_prefix("file:") {
        let path = PathBuf::from(path);
        if let Some(name) = sidecar_name(&path) {
            return name;
        }
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            return assets::pretty_name(stem);
        }
    }
    "Custom crosshair".to_string()
}

fn sidecar_name(path: &Path) -> Option<String> {
    let sidecar = path.with_extension("toml");
    let text = fs::read_to_string(sidecar).ok()?;
    let design: DesignerDesign = toml::from_str(&text).ok()?;
    let name = design.name.trim();
    (!name.is_empty()).then(|| name.to_string())
}

fn is_image(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("svg" | "png")
    )
}

/// Scan the per-user crosshair folder (and an optional extra folder) for
/// importable images.
pub fn custom_crosshairs(extra_dir: Option<&str>) -> Vec<(String, String)> {
    let _ = config::ensure_dirs();
    let mut folders = vec![config::custom_crosshair_dir()];
    if let Some(extra) = extra_dir.map(str::trim).filter(|s| !s.is_empty()) {
        folders.push(PathBuf::from(extra));
    }

    let mut found: Vec<PathBuf> = Vec::new();
    for folder in folders {
        let Ok(entries) = fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && is_image(&path) {
                // Ignore duplicate paths coming from the same folder being
                // listed twice.
                if !found.iter().any(|p| p == &path) {
                    found.push(path);
                }
            }
        }
    }

    let mut items: Vec<(String, String)> = found
        .into_iter()
        .map(|path| {
            let source = format!("file:{}", path.display());
            let label = label_for_source(&source);
            (source, label)
        })
        .collect();
    items.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));
    items
}

fn slugify(name: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "custom".to_string()
    } else {
        out
    }
}

fn unique_path(dir: &Path, slug: &str, extension: &str) -> PathBuf {
    let mut candidate = dir.join(format!("{slug}.{extension}"));
    let mut index = 2;
    while candidate.exists() {
        candidate = dir.join(format!("{slug}-{index}.{extension}"));
        index += 1;
    }
    candidate
}

/// Copy an image into the user crosshair folder after checking that it can be
/// rendered. Returns the paths of the imported files.
pub fn import_files(paths: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    config::ensure_dirs().map_err(|e| format!("create the crosshair folder: {e}"))?;
    let custom_dir = config::custom_crosshair_dir();
    let mut imported = Vec::new();

    for path in paths {
        if !is_image(path) {
            return Err(format!(
                "{} is not an SVG or PNG file",
                path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("file")
            ));
        }
        if render::load_source(&Source::File(path.clone()), 32).is_none() {
            return Err(format!(
                "{} could not be rendered as an image",
                path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("file")
            ));
        }

        if path.parent() == Some(custom_dir.as_path()) {
            imported.push(path.clone());
            continue;
        }

        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("custom");
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("svg")
            .to_ascii_lowercase();
        let destination = unique_path(&custom_dir, &slugify(stem), &extension);
        fs::copy(path, &destination)
            .map_err(|e| format!("copy {}: {e}", path.display()))?;
        imported.push(destination);
    }

    Ok(imported)
}

/// Save a designer crosshair plus a TOML sidecar (used to keep the display
/// name and allow editing later).
pub fn save_design(design: &DesignerDesign, display_name: &str) -> Result<PathBuf, String> {
    config::ensure_dirs().map_err(|e| format!("create the crosshair folder: {e}"))?;
    let name = display_name.trim();
    if name.is_empty() {
        return Err("Give the crosshair a name first".to_string());
    }

    let custom_dir = config::custom_crosshair_dir();
    let slug = slugify(name);
    let path = unique_path(&custom_dir, &slug, "svg");
    let svg = design.to_svg();
    if render::load_source(&Source::Svg(svg.clone()), 32).is_none() {
        return Err("The designer output could not be rendered".to_string());
    }
    fs::write(&path, svg).map_err(|e| format!("write {}: {e}", path.display()))?;

    let mut sidecar = design.clone();
    sidecar.name = name.to_string();
    let text = toml::to_string_pretty(&sidecar)
        .map_err(|e| format!("serialize designer settings: {e}"))?;
    fs::write(path.with_extension("toml"), text)
        .map_err(|e| format!("write designer sidecar: {e}"))?;
    Ok(path)
}

/// Theme-aware tint used for white built-in SVGs.
pub fn theme_tint(theme: crate::model::Theme) -> [u8; 4] {
    let palette = crate::ui::theme::palette(theme);
    hex_to_rgba(Some(palette.text), [255, 255, 255, 255])
}
