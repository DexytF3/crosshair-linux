use serde::{Deserialize, Serialize};

pub const APP_NAME: &str = "Crosshair Linux";
pub const APP_ID: &str = "io.github.crosshairlinux.CrosshairLinux";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DEFAULT_TOGGLE_HOTKEY: &str = "Ctrl+Shift+X";
pub const DEFAULT_CROSSHAIR: &str = "builtin:cross-dot";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Style {
    pub size: u32,
    pub hue: f32,
    pub rotation: f32,
    pub opacity: f32,
    /// `builtin:<name>` or `file:<absolute path>`
    pub crosshair: String,
    pub fill_color: Option<String>,
    pub outline: bool,
    pub outline_width: u32,
    pub outline_color: String,
    pub glow: bool,
    pub glow_color: String,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            size: 48,
            hue: 0.0,
            rotation: 0.0,
            opacity: 1.0,
            crosshair: DEFAULT_CROSSHAIR.to_string(),
            fill_color: None,
            outline: false,
            outline_width: 2,
            outline_color: "#000000".to_string(),
            glow: false,
            glow_color: "#6c8cff".to_string(),
        }
    }
}

impl Style {
    pub fn clamped_size(&self) -> u32 {
        self.size.clamp(16, 256)
    }

    pub fn clamped_opacity(&self) -> f32 {
        self.opacity.clamp(0.05, 1.0)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum PositionMode {
    #[default]
    Center,
    Pixel,
    Follow,
}

impl PositionMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Center => "Center",
            Self::Pixel => "Custom",
            Self::Follow => "Follow cursor",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Position {
    pub mode: PositionMode,
    /// Offset of the crosshair center inside the selected display.
    pub x: i32,
    pub y: i32,
    /// Stable display name (for example `DP-1`). `None` means the primary display.
    pub display: Option<String>,
}

impl Default for Position {
    fn default() -> Self {
        Self {
            mode: PositionMode::Center,
            x: 0,
            y: 0,
            display: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Dark,
    Amoled,
    Light,
}

impl Theme {
    pub fn label(self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Amoled => "AMOLED dark",
            Self::Light => "Light",
        }
    }

    pub const ALL: [Theme; 3] = [Theme::Dark, Theme::Amoled, Theme::Light];
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    pub style: Style,
    pub position: Position,
    pub overlay_on: bool,
    pub show_on_all_displays: bool,
    pub tray: bool,
    pub theme: Theme,
    pub toggle_hotkey: String,
    pub custom_dir: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            style: Style::default(),
            position: Position::default(),
            overlay_on: false,
            show_on_all_displays: false,
            tray: true,
            theme: Theme::default(),
            toggle_hotkey: DEFAULT_TOGGLE_HOTKEY.to_string(),
            custom_dir: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DisplayInfo {
    /// Stable identifier. Wayland uses the output name, X11 uses the RandR monitor name.
    pub id: String,
    /// User-facing name.
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub primary: bool,
    /// Wayland output scale (1 on X11).
    pub scale: i32,
}

impl DisplayInfo {
    pub fn label(&self, index: usize) -> String {
        if self.name.trim().is_empty() {
            format!("Display {}", index + 1)
        } else {
            self.name.clone()
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderSpec {
    pub style: Style,
    pub size: u32,
}

impl RenderSpec {
    pub fn new(style: &Style) -> Self {
        Self {
            style: style.clone(),
            size: style.clamped_size(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum DesignerCap {
    Butt,
    Round,
    Square,
}

impl DesignerCap {
    pub fn svg_value(self) -> &'static str {
        match self {
            Self::Butt => "butt",
            Self::Round => "round",
            Self::Square => "square",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Butt => "Butt",
            Self::Round => "Round",
            Self::Square => "Square",
        }
    }

    pub const ALL: [DesignerCap; 3] = [DesignerCap::Butt, DesignerCap::Round, DesignerCap::Square];
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct DesignerDesign {
    pub name: String,
    pub arm_length: f32,
    pub gap: f32,
    pub thickness: f32,
    pub dot_radius: f32,
    pub ring_radius: f32,
    pub ring_thickness: f32,
    pub cap: String,
    pub up: bool,
    pub right: bool,
    pub down: bool,
    pub left: bool,
    pub diagonal: bool,
}

impl Default for DesignerDesign {
    fn default() -> Self {
        Self {
            name: String::new(),
            arm_length: 10.0,
            gap: 4.0,
            thickness: 2.0,
            dot_radius: 1.5,
            ring_radius: 0.0,
            ring_thickness: 1.5,
            cap: "butt".to_string(),
            up: true,
            right: true,
            down: true,
            left: true,
            diagonal: false,
        }
    }
}

impl DesignerDesign {
    pub fn cap_enum(&self) -> DesignerCap {
        match self.cap.as_str() {
            "round" => DesignerCap::Round,
            "square" => DesignerCap::Square,
            _ => DesignerCap::Butt,
        }
    }

    /// Build a simple, clean SVG on a 32x32 viewBox. Shapes are white so the
    /// normal hue/solid-fill styling pipeline can recolor them.
    pub fn to_svg(&self) -> String {
        let mut out = String::new();
        out.push_str(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32" fill="none" stroke="white" stroke-linecap=""#,
        );
        out.push_str(self.cap_enum().svg_value());
        out.push_str(r#"" stroke-linejoin="round">"#);

        let cx = 16.0_f32;
        let cy = 16.0_f32;
        let gap = self.gap.clamp(0.0, 15.0);
        let len = self.arm_length.clamp(0.0, 16.0);
        let thickness = self.thickness.clamp(0.25, 8.0);
        let start = gap;
        let end = (gap + len).min(16.0);

        if len > 0.0 {
            let mut line = |x1: f32, y1: f32, x2: f32, y2: f32| {
                out.push_str(&format!(
                    r#"<line x1="{x1:.2}" y1="{y1:.2}" x2="{x2:.2}" y2="{y2:.2}" stroke-width="{thickness:.2}"/>"#
                ));
            };
            if self.diagonal {
                let d = start * 0.70710678;
                let e = end * 0.70710678;
                if self.up || self.left {
                    line(cx - d, cy - d, cx - e, cy - e);
                }
                if self.up || self.right {
                    line(cx + d, cy - d, cx + e, cy - e);
                }
                if self.down || self.left {
                    line(cx - d, cy + d, cx - e, cy + e);
                }
                if self.down || self.right {
                    line(cx + d, cy + d, cx + e, cy + e);
                }
            } else {
                if self.up {
                    line(cx, cy - start, cx, cy - end);
                }
                if self.down {
                    line(cx, cy + start, cx, cy + end);
                }
                if self.left {
                    line(cx - start, cy, cx - end, cy);
                }
                if self.right {
                    line(cx + start, cy, cx + end, cy);
                }
            }
        }

        let ring_radius = self.ring_radius.clamp(0.0, 15.0);
        if ring_radius > 0.0 {
            out.push_str(&format!(
                r#"<circle cx="{cx}" cy="{cy}" r="{ring_radius:.2}" stroke-width="{:.2}"/>"#,
                self.ring_thickness.clamp(0.25, 8.0)
            ));
        }

        let dot = self.dot_radius.clamp(0.0, 8.0);
        if dot > 0.0 {
            // A tiny dark rim keeps the dot readable on bright backgrounds.
            out.push_str(&format!(
                r#"<circle cx="{cx}" cy="{cy}" r="{:.2}" fill="white" stroke="black" stroke-width="{:.2}"/>"#,
                dot,
                (thickness * 0.25).clamp(0.0, 0.5)
            ));
        }

        out.push_str("</svg>");
        out
    }
}

pub fn hex_to_rgba(hex: Option<&str>, fallback: [u8; 4]) -> [u8; 4] {
    let Some(hex) = hex else { return fallback };
    let h = hex.trim().trim_start_matches('#');
    let parse = |s: &str| u8::from_str_radix(s, 16).ok();
    match h.len() {
        6 => match (parse(&h[0..2]), parse(&h[2..4]), parse(&h[4..6])) {
            (Some(r), Some(g), Some(b)) => [r, g, b, 255],
            _ => fallback,
        },
        8 => match (
            parse(&h[0..2]),
            parse(&h[2..4]),
            parse(&h[4..6]),
            parse(&h[6..8]),
        ) {
            (Some(r), Some(g), Some(b), Some(a)) => [r, g, b, a],
            _ => fallback,
        },
        _ => fallback,
    }
}

pub fn rgba_to_hex(c: [u8; 4]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}
