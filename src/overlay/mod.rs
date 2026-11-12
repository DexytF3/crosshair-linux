//! Overlay backends.
//!
//! The overlay lives in its own thread and never runs a render loop. It renders
//! once per settings change and sleeps until the next command. The only
//! exception is follow-cursor mode on X11, which reacts to pointer motion
//! events instead of polling.

pub mod wayland;
pub mod x11;

use crate::model::{DisplayInfo, Position, Style};
use calloop::channel::{channel, Channel, Sender};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    X11,
    Wayland,
}

impl Backend {
    pub fn label(self) -> &'static str {
        match self {
            Self::X11 => "X11 / XWayland",
            Self::Wayland => "Wayland (wlr-layer-shell)",
        }
    }

    pub fn supports_follow(self) -> bool {
        matches!(self, Self::X11)
    }
}

impl fmt::Display for Backend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone)]
pub struct OverlaySpec {
    pub style: Style,
    pub position: Position,
    pub show_all: bool,
}

impl OverlaySpec {
    pub fn new(style: Style, position: Position, show_all: bool) -> Self {
        Self {
            style,
            position,
            show_all,
        }
    }
}

#[derive(Debug, Clone)]
pub enum OverlayCommand {
    Update { spec: OverlaySpec, visible: bool },
    Shutdown,
}

#[derive(Clone)]
pub struct OverlayHandle {
    pub backend: Backend,
    tx: Sender<OverlayCommand>,
}

impl OverlayHandle {
    pub fn update(&self, spec: OverlaySpec, visible: bool) {
        let _ = self.tx.send(OverlayCommand::Update { spec, visible });
    }

    pub fn shutdown(&self) {
        let _ = self.tx.send(OverlayCommand::Shutdown);
    }
}

/// Returns the preferred backend for this session.
///
/// Native Wayland is preferred when the compositor implements
/// `zwlr_layer_shell_v1`. GNOME does not, so it falls back to XWayland. The
/// `CROSSHAIR_BACKEND` environment variable can force `x11` or `wayland`.
pub fn detect_backend() -> Option<Backend> {
    let forced = std::env::var("CROSSHAIR_BACKEND")
        .ok()
        .map(|v| v.trim().to_ascii_lowercase());
    match forced.as_deref() {
        Some("x11") | Some("xwayland") => {
            return x11::available().then_some(Backend::X11);
        }
        Some("wayland") | Some("wlr") => {
            return wayland::available().then_some(Backend::Wayland);
        }
        _ => {}
    }

    if std::env::var_os("WAYLAND_DISPLAY").is_some() && wayland::available() {
        return Some(Backend::Wayland);
    }
    if std::env::var_os("DISPLAY").is_some() && x11::available() {
        return Some(Backend::X11);
    }
    None
}

pub fn list_displays(backend: Backend) -> Result<Vec<DisplayInfo>, String> {
    match backend {
        Backend::X11 => x11::list_displays(),
        Backend::Wayland => wayland::list_displays(),
    }
}

/// Start the overlay thread. The initial state is applied right away.
pub fn spawn(
    backend: Backend,
    spec: OverlaySpec,
    visible: bool,
) -> Result<OverlayHandle, String> {
    let (tx, rx) = channel::<OverlayCommand>();
    let handle = OverlayHandle {
        backend,
        tx: tx.clone(),
    };
    let thread_name = match backend {
        Backend::X11 => "crosshair-overlay-x11",
        Backend::Wayland => "crosshair-overlay-wayland",
    };
    std::thread::Builder::new()
        .name(thread_name.to_string())
        .spawn(move || {
            let result = match backend {
                Backend::X11 => x11::run(rx, spec, visible),
                Backend::Wayland => wayland::run(rx, spec, visible),
            };
            if let Err(err) = result {
                eprintln!("crosshair-linux: overlay backend failed: {err}");
            }
        })
        .map_err(|e| format!("spawn overlay thread: {e}"))?;
    Ok(handle)
}

/// Compute the crosshair window origin relative to the top-left corner of a
/// display for the static position modes. Follow mode is handled by the
/// backend because it needs the pointer position.
pub fn local_origin(display: &DisplayInfo, size: u32, position: &Position) -> (i32, i32) {
    let s = size as i32;
    let (base_x, base_y) = match position.mode {
        crate::model::PositionMode::Center => (
            (display.width as i32 - s) / 2,
            (display.height as i32 - s) / 2,
        ),
        crate::model::PositionMode::Pixel => (position.x - s / 2, position.y - s / 2),
        crate::model::PositionMode::Follow => (
            (display.width as i32 - s) / 2,
            (display.height as i32 - s) / 2,
        ),
    };
    match position.mode {
        crate::model::PositionMode::Center | crate::model::PositionMode::Follow => {
            (base_x + position.x, base_y + position.y)
        }
        crate::model::PositionMode::Pixel => (base_x, base_y),
    }
}

/// Pick the display targeted by a non-"all displays" configuration.
pub fn target_display<'a>(
    displays: &'a [DisplayInfo],
    position: &Position,
) -> Option<&'a DisplayInfo> {
    if let Some(id) = position.display.as_deref() {
        if let Some(found) = displays.iter().find(|d| d.id == id) {
            return Some(found);
        }
    }
    displays
        .iter()
        .find(|d| d.primary)
        .or_else(|| displays.first())
}

pub fn followed_origin(display: &DisplayInfo, size: u32, position: &Position, pointer: (i32, i32)) -> (i32, i32) {
    let s = size as i32;
    let _ = display;
    (
        pointer.0 - s / 2 + position.x,
        pointer.1 - s / 2 + position.y,
    )
}

/// Keep a channel receiver alive in the public API even though backends consume
/// it internally. This is useful for feature detection and tests.
#[allow(dead_code)]
pub(crate) fn _rx_type_check(_rx: Channel<OverlayCommand>) {}
