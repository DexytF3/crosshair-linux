//! Main application controller, coordinating config persistence, the overlay
//! backend, global hotkeys, the system tray, and the settings window UI.

use crate::action::AppAction;
use crate::hotkeys::{self, HotkeyBackend, HotkeyService};
use crate::model::{Config, DisplayInfo, Position, PositionMode, Style, Theme};
use crate::overlay::{self, Backend, OverlayHandle, OverlaySpec};
use crate::tray::{self, TrayHandle};
use crate::ui::{self, window::Ui};
use gtk4::prelude::*;
use gtk4::glib;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use async_channel::Sender;

pub struct UiSnapshot {
    pub config: Config,
    pub backend: Option<Backend>,
    pub displays: Vec<DisplayInfo>,
    pub effective_display: Option<String>,
    pub wayland_session: bool,
    pub follow_available: bool,
    pub hotkey_backend: Option<HotkeyBackend>,
    pub hotkey_error: Option<String>,
}

struct ControllerInner {
    app: gtk4::Application,
    config: RefCell<Config>,
    backend: Option<Backend>,
    displays: RefCell<Vec<DisplayInfo>>,
    wayland_session: bool,
    overlay: RefCell<Option<OverlayHandle>>,
    hotkey: RefCell<Option<HotkeyService>>,
    hotkey_backend: Cell<Option<HotkeyBackend>>,
    hotkey_error: RefCell<Option<String>>,
    tray: RefCell<Option<TrayHandle>>,
    ui: RefCell<Option<Rc<Ui>>>,
    tx: Sender<AppAction>,
    save_source: RefCell<Option<glib::SourceId>>,
    css_provider: gtk4::CssProvider,
}

#[derive(Clone)]
pub struct Controller {
    inner: Arc<ControllerInner>,
}

impl Controller {
    pub fn new(app: gtk4::Application, tx: Sender<AppAction>) -> Self {
        let config = Config::load();
        let backend = overlay::detect_backend();
        let displays = backend.and_then(|b| overlay::list_displays(b).ok()).unwrap_or_default();
        let wayland_session = std::env::var_os("WAYLAND_DISPLAY").is_some();

        let css_provider = gtk4::CssProvider::new();
        ui::theme::apply(&css_provider, config.theme);
        if let Some(display) = gtk4::gdk::Display::default() {
            gtk4::style_context_add_provider_for_display(
                &display,
                &css_provider,
                // Some distributions/themes install their palette as a user
                // stylesheet (~/.config/gtk-4.0/gtk.css). Go just above the
                // user priority so our own theme always wins.
                gtk4::STYLE_PROVIDER_PRIORITY_USER + 10,
            );
        }

        let inner = Arc::new(ControllerInner {
            app,
            config: RefCell::new(config),
            backend,
            displays: RefCell::new(displays),
            wayland_session,
            overlay: RefCell::new(None),
            hotkey: RefCell::new(None),
            hotkey_backend: Cell::new(None),
            hotkey_error: RefCell::new(None),
            tray: RefCell::new(None),
            ui: RefCell::new(None),
            tx,
            save_source: RefCell::new(None),
            css_provider,
        });
        Self { inner }
    }

    pub fn app(&self) -> &gtk4::Application {
        &self.inner.app
    }

    pub fn config(&self) -> Config {
        self.inner.config.borrow().clone()
    }

    pub fn snapshot(&self) -> UiSnapshot {
        let config = self.inner.config.borrow().clone();
        let displays = self.inner.displays.borrow().clone();
        let effective = overlay::target_display(&displays, &config.position).map(|d| d.id.clone());
        let follow_available = self.inner.backend.map_or(false, Backend::supports_follow) && !self.inner.wayland_session;
        UiSnapshot {
            config,
            backend: self.inner.backend,
            displays,
            effective_display: effective,
            wayland_session: self.inner.wayland_session,
            follow_available,
            hotkey_backend: self.inner.hotkey_backend.get(),
            hotkey_error: self.inner.hotkey_error.borrow().clone(),
        }
    }

    pub fn bind_ui(&self, ui: Rc<Ui>) {
        *self.inner.ui.borrow_mut() = Some(ui);
        self.refresh_ui();
        self.sync_overlay();
        self.sync_tray();
        self.sync_hotkeys();
    }

    pub fn refresh_ui(&self) {
        if let Some(ui) = self.inner.ui.borrow().as_ref() {
            ui::window::refresh(self, ui);
        }
    }

    // -----------------------------------------------------------------------
    // Actions & Events
    // -----------------------------------------------------------------------

    pub fn handle_action(&self, action: AppAction) {
        match action {
            AppAction::Toggle => {
                let mut cfg = self.inner.config.borrow_mut();
                cfg.overlay_on = !cfg.overlay_on;
                let active = cfg.overlay_on;
                drop(cfg);
                self.schedule_save();
                self.sync_overlay();
                self.refresh_ui();
                self.sync_tray();
                let state_name = if active { "visible" } else { "hidden" };
                self.notice(format!("Crosshair is now {state_name}."));
            }
            AppAction::Show => {
                let mut cfg = self.inner.config.borrow_mut();
                if !cfg.overlay_on {
                    cfg.overlay_on = true;
                    drop(cfg);
                    self.schedule_save();
                    self.sync_overlay();
                    self.refresh_ui();
                    self.sync_tray();
                }
            }
            AppAction::Hide => {
                let mut cfg = self.inner.config.borrow_mut();
                if cfg.overlay_on {
                    cfg.overlay_on = false;
                    drop(cfg);
                    self.schedule_save();
                    self.sync_overlay();
                    self.refresh_ui();
                    self.sync_tray();
                }
            }
            AppAction::OpenSettings => {
                self.present_settings();
            }
            AppAction::Quit => {
                self.quit();
            }
            AppAction::Notice(msg) => {
                self.notice(msg);
            }
        }
    }

    pub fn present_settings(&self) {
        if let Some(ui) = self.inner.ui.borrow().as_ref() {
            ui.window.present();
        }
    }

    pub fn notice(&self, message: impl Into<String>) {
        let msg = message.into();
        eprintln!("crosshair-linux: {msg}");
        if let Some(ui) = self.inner.ui.borrow().as_ref() {
            ui::window::show_notice(ui, &msg);
        }
    }

    pub fn quit(&self) {
        self.flush_save();
        self.shutdown_overlay();
        self.inner.hotkey.borrow_mut().take();
        if let Some(tray) = self.inner.tray.borrow_mut().take() {
            tray.shutdown();
        }
        self.inner.app.quit();
    }

    // -----------------------------------------------------------------------
    // State mutators
    // -----------------------------------------------------------------------

    pub fn set_enabled(&self, enabled: bool) {
        let mut cfg = self.inner.config.borrow_mut();
        if cfg.overlay_on == enabled {
            return;
        }
        cfg.overlay_on = enabled;
        drop(cfg);
        self.schedule_save();
        self.sync_overlay();
        self.refresh_ui();
        self.sync_tray();
    }

    pub fn update_style(&self, f: impl FnOnce(&mut Style)) {
        {
            let mut cfg = self.inner.config.borrow_mut();
            f(&mut cfg.style);
        }
        self.schedule_save();
        self.sync_overlay();
        self.refresh_ui();
    }

    pub fn set_crosshair(&self, source: &str) {
        {
            let mut cfg = self.inner.config.borrow_mut();
            cfg.style.crosshair = source.to_string();
        }
        self.schedule_save();
        self.sync_overlay();
        self.refresh_ui();
    }

    pub fn reload_crosshairs(&self) {
        let displays = self.inner.backend.and_then(|b| overlay::list_displays(b).ok()).unwrap_or_default();
        *self.inner.displays.borrow_mut() = displays;
        if let Some(ui) = self.inner.ui.borrow().as_ref() {
            ui::window::reload_crosshairs(self, ui);
        }
    }

    pub fn set_show_all(&self, show_all: bool) {
        {
            let mut cfg = self.inner.config.borrow_mut();
            cfg.show_on_all_displays = show_all;
        }
        self.schedule_save();
        self.sync_overlay();
        self.refresh_ui();
    }

    pub fn set_display(&self, display: Option<String>) {
        {
            let mut cfg = self.inner.config.borrow_mut();
            cfg.position.display = display;
        }
        self.schedule_save();
        self.sync_overlay();
        self.refresh_ui();
    }

    pub fn set_position_mode(&self, mode: PositionMode) {
        {
            let mut cfg = self.inner.config.borrow_mut();
            if mode == PositionMode::Follow && (self.inner.wayland_session || !self.inner.backend.map_or(false, Backend::supports_follow)) {
                return;
            }
            cfg.position.mode = mode;
        }
        self.schedule_save();
        self.sync_overlay();
        self.refresh_ui();
    }

    pub fn update_position(&self, f: impl FnOnce(&mut Position)) {
        {
            let mut cfg = self.inner.config.borrow_mut();
            f(&mut cfg.position);
        }
        self.schedule_save();
        self.sync_overlay();
        self.refresh_ui();
    }

    pub fn set_theme(&self, theme: Theme) {
        {
            let mut cfg = self.inner.config.borrow_mut();
            cfg.theme = theme;
        }
        self.schedule_save();
        ui::theme::apply(&self.inner.css_provider, theme);
        self.refresh_ui();
    }

    pub fn set_tray_enabled(&self, enabled: bool) {
        {
            let mut cfg = self.inner.config.borrow_mut();
            cfg.tray = enabled;
        }
        self.schedule_save();
        self.sync_tray();
        self.refresh_ui();
    }

    pub fn apply_hotkey_entry(&self, entry: &gtk4::Entry) {
        let text = entry.text().trim().to_string();
        {
            let mut cfg = self.inner.config.borrow_mut();
            cfg.toggle_hotkey = text;
        }
        self.schedule_save();
        self.sync_hotkeys();
        self.refresh_ui();
    }

    pub fn reset_style(&self) {
        {
            let mut cfg = self.inner.config.borrow_mut();
            let crosshair = cfg.style.crosshair.clone();
            cfg.style = Style::default();
            cfg.style.crosshair = crosshair;
        }
        self.schedule_save();
        self.sync_overlay();
        self.refresh_ui();
        self.notice("Style reset to defaults.");
    }

    pub fn show_about(&self) {
        let ui = self.inner.ui.borrow().as_ref().cloned();
        let Some(ui) = ui else {
            return;
        };
        ui::about::present(&ui.window);
    }

    // -----------------------------------------------------------------------
    // Orchestration & Services
    // -----------------------------------------------------------------------

    fn sync_overlay(&self) {
        let cfg = self.inner.config.borrow();
        let backend = self.inner.backend;
        let visible = cfg.overlay_on;
        let spec = OverlaySpec::new(cfg.style.clone(), cfg.position.clone(), cfg.show_on_all_displays);
        drop(cfg);

        if !visible {
            self.shutdown_overlay();
            return;
        }

        let Some(backend) = backend else {
            self.shutdown_overlay();
            return;
        };

        let mut overlay_ref = self.inner.overlay.borrow_mut();
        if let Some(handle) = overlay_ref.as_ref() {
            if handle.backend == backend {
                handle.update(spec, visible);
                return;
            }
            handle.shutdown();
            *overlay_ref = None;
        }

        match overlay::spawn(backend, spec, visible) {
            Ok(handle) => {
                *overlay_ref = Some(handle);
            }
            Err(err) => {
                self.notice(format!("Could not start overlay: {err}"));
            }
        }
    }

    fn shutdown_overlay(&self) {
        if let Some(handle) = self.inner.overlay.borrow_mut().take() {
            handle.shutdown();
        }
    }

    fn sync_hotkeys(&self) {
        let hotkey_str = self.inner.config.borrow().toggle_hotkey.clone();
        self.inner.hotkey.borrow_mut().take();
        self.inner.hotkey_backend.set(None);
        self.inner.hotkey_error.borrow_mut().take();

        if hotkey_str.trim().is_empty() {
            return;
        }

        match hotkeys::start(&hotkey_str, self.inner.tx.clone()) {
            Ok(service) => {
                self.inner.hotkey_backend.set(Some(service.backend));
                *self.inner.hotkey.borrow_mut() = Some(service);
            }
            Err(err) => {
                *self.inner.hotkey_error.borrow_mut() = Some(err.clone());
                self.notice(err);
            }
        }
    }

    fn sync_tray(&self) {
        let (enabled, visible, backend_label, hotkey) = {
            let cfg = self.inner.config.borrow();
            let backend_str = self.inner.backend.map_or("No backend", Backend::label);
            (cfg.tray, cfg.overlay_on, backend_str, cfg.toggle_hotkey.clone())
        };

        if !enabled {
            if let Some(tray) = self.inner.tray.borrow_mut().take() {
                tray.shutdown();
            }
            return;
        }

        let tray_ref = self.inner.tray.borrow();
        if let Some(tray) = tray_ref.as_ref() {
            tray.set_visible(visible);
            tray.set_status(backend_label, &hotkey);
            return;
        }
        drop(tray_ref);

        match tray::spawn(self.inner.tx.clone(), visible, backend_label, &hotkey) {
            Ok(tray) => {
                *self.inner.tray.borrow_mut() = Some(tray);
            }
            Err(err) => {
                // Tray failure on desktops without SNI host is non-fatal.
                eprintln!("crosshair-linux: tray warning: {err}");
            }
        }
    }

    fn schedule_save(&self) {
        if let Some(source) = self.inner.save_source.borrow_mut().take() {
            remove_source(source);
        }
        let inner = self.inner.clone();
        let source = glib::timeout_add_local_once(std::time::Duration::from_millis(400), move || {
            inner.save_source.borrow_mut().take();
            let cfg = inner.config.borrow().clone();
            if let Err(err) = cfg.save() {
                eprintln!("crosshair-linux: save config: {err}");
            }
        });
        *self.inner.save_source.borrow_mut() = Some(source);
    }

    fn flush_save(&self) {
        if let Some(source) = self.inner.save_source.borrow_mut().take() {
            remove_source(source);
        }
        let cfg = self.inner.config.borrow().clone();
        if let Err(err) = cfg.save() {
            eprintln!("crosshair-linux: save config: {err}");
        }
    }
}

fn remove_source(source: glib::SourceId) {
    // `SourceId::remove` panics when the source already fired. Calling the
    // FFI directly keeps shutdown safe in that case.
    unsafe {
        glib::ffi::g_source_remove(source.as_raw());
    }
}
