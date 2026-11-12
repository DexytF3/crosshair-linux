//! StatusNotifierItem tray icon, implemented with `ksni`.
//!
//! The tray is optional: on desktops without an SNI host (GNOME without the
//! AppIndicator extension, for example) `spawn` still succeeds, but the icon
//! simply never appears.

use crate::action::AppAction;
use crate::render;
use ksni::blocking::{Handle, TrayMethods};
use ksni::menu::{MenuItem, StandardItem};

pub struct CrosshairTray {
    tx: async_channel::Sender<AppAction>,
    visible: bool,
    backend: String,
    hotkey: String,
    icon: Vec<ksni::Icon>,
}

impl CrosshairTray {
    fn action(&self, action: AppAction) {
        let _ = self.tx.try_send(action);
    }
}

fn tray_icons() -> Vec<ksni::Icon> {
    [22u32, 32, 48, 64]
        .into_iter()
        .map(|size| {
            let pixmap = render::brand_icon(size);
            ksni::Icon {
                width: size as i32,
                height: size as i32,
                data: render::pixmap_to_argb_network(&pixmap),
            }
        })
        .collect()
}

impl ksni::Tray for CrosshairTray {
    const MENU_ON_ACTIVATE: bool = false;

    fn id(&self) -> String {
        "crosshair-linux".to_string()
    }

    fn title(&self) -> String {
        "Crosshair Linux".to_string()
    }

    fn icon_name(&self) -> String {
        "crosshair-linux".to_string()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        self.icon.clone()
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        self.action(AppAction::OpenSettings);
    }

    fn secondary_activate(&mut self, _x: i32, _y: i32) {
        self.action(AppAction::Toggle);
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        let state = if self.visible { "Visible" } else { "Hidden" };
        ksni::ToolTip {
            title: "Crosshair Linux".to_string(),
            description: format!(
                "{state} · {backend} · {hotkey}\nClick to open settings, middle-click to toggle.",
                backend = self.backend,
                hotkey = self.hotkey
            ),
            icon_pixmap: self.icon.clone(),
            ..Default::default()
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            StandardItem {
                label: "Open settings".to_string(),
                activate: Box::new(|tray: &mut Self| tray.action(AppAction::OpenSettings)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: if self.visible {
                    "Hide crosshair".to_string()
                } else {
                    "Show crosshair".to_string()
                },
                activate: Box::new(|tray: &mut Self| tray.action(AppAction::Toggle)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: format!("Hotkey: {}", self.hotkey),
                enabled: false,
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit".to_string(),
                activate: Box::new(|tray: &mut Self| tray.action(AppAction::Quit)),
                ..Default::default()
            }
            .into(),
        ]
    }
}

pub struct TrayHandle {
    handle: Handle<CrosshairTray>,
}

impl TrayHandle {
    pub fn set_visible(&self, visible: bool) {
        let _ = self.handle.update(|tray| tray.visible = visible);
    }

    pub fn set_status(&self, backend: &str, hotkey: &str) {
        let _ = self.handle.update(|tray| {
            tray.backend = backend.to_string();
            tray.hotkey = hotkey.to_string();
        });
    }

    pub fn shutdown(&self) {
        self.handle.shutdown().wait();
    }
}

pub fn spawn(
    tx: async_channel::Sender<AppAction>,
    visible: bool,
    backend: &str,
    hotkey: &str,
) -> Result<TrayHandle, String> {
    let tray = CrosshairTray {
        tx,
        visible,
        backend: backend.to_string(),
        hotkey: hotkey.to_string(),
        icon: tray_icons(),
    };
    // Do not fail the whole app when no SNI watcher is present (common on
    // GNOME); the window and hotkeys still work.
    let handle = tray
        .assume_sni_available(true)
        .spawn()
        .map_err(|e| format!("start system tray: {e}"))?;
    Ok(TrayHandle { handle })
}
