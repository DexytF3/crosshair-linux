//! Global hotkeys.
//!
//! X11 sessions use passive key grabs (`XGrabKey`) on the root window inside a
//! small calloop event loop. Wayland sessions use the
//! `org.freedesktop.portal.GlobalShortcuts` D-Bus portal, because a client
//! cannot grab keys globally by itself there. Both backends emit
//! [`AppAction::Toggle`] through an async channel so the GTK main loop never
//! blocks.

use crate::action::AppAction;
use std::fmt;
use std::time::{Duration, Instant};

const TOGGLE_DEBOUNCE: Duration = Duration::from_millis(300);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Key {
    Char(char),
    Function(u8),
    Named(NamedKey),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NamedKey {
    Space,
    Tab,
    Enter,
    Escape,
    Backspace,
    Delete,
    Insert,
    Home,
    End,
    PageUp,
    PageDown,
    Left,
    Up,
    Right,
    Down,
    Minus,
    Equal,
    BracketLeft,
    BracketRight,
    Backslash,
    Semicolon,
    Quote,
    Comma,
    Period,
    Slash,
    Backquote,
}

impl NamedKey {
    fn parse(token: &str) -> Option<Self> {
        Some(match token {
            "space" | "spacebar" => Self::Space,
            "tab" => Self::Tab,
            "enter" | "return" => Self::Enter,
            "esc" | "escape" => Self::Escape,
            "backspace" | "back" => Self::Backspace,
            "delete" | "del" => Self::Delete,
            "insert" | "ins" => Self::Insert,
            "home" => Self::Home,
            "end" => Self::End,
            "pageup" | "pgup" => Self::PageUp,
            "pagedown" | "pgdn" => Self::PageDown,
            "left" | "arrowleft" => Self::Left,
            "up" | "arrowup" => Self::Up,
            "right" | "arrowright" => Self::Right,
            "down" | "arrowdown" => Self::Down,
            "minus" | "dash" => Self::Minus,
            "equal" | "equals" => Self::Equal,
            "bracketleft" | "leftbracket" => Self::BracketLeft,
            "bracketright" | "rightbracket" => Self::BracketRight,
            "backslash" => Self::Backslash,
            "semicolon" => Self::Semicolon,
            "quote" | "apostrophe" => Self::Quote,
            "comma" => Self::Comma,
            "period" | "dot" => Self::Period,
            "slash" => Self::Slash,
            "backquote" | "grave" => Self::Backquote,
            _ => return None,
        })
    }

    fn label(self) -> &'static str {
        match self {
            Self::Space => "Space",
            Self::Tab => "Tab",
            Self::Enter => "Enter",
            Self::Escape => "Escape",
            Self::Backspace => "Backspace",
            Self::Delete => "Delete",
            Self::Insert => "Insert",
            Self::Home => "Home",
            Self::End => "End",
            Self::PageUp => "Page Up",
            Self::PageDown => "Page Down",
            Self::Left => "Left",
            Self::Up => "Up",
            Self::Right => "Right",
            Self::Down => "Down",
            Self::Minus => "Minus",
            Self::Equal => "Equal",
            Self::BracketLeft => "Left Bracket",
            Self::BracketRight => "Right Bracket",
            Self::Backslash => "Backslash",
            Self::Semicolon => "Semicolon",
            Self::Quote => "Quote",
            Self::Comma => "Comma",
            Self::Period => "Period",
            Self::Slash => "Slash",
            Self::Backquote => "Backquote",
        }
    }

    /// XKB keysym name used by the XDG shortcuts specification.
    fn portal_name(self) -> &'static str {
        match self {
            Self::Space => "space",
            Self::Tab => "Tab",
            Self::Enter => "Return",
            Self::Escape => "Escape",
            Self::Backspace => "BackSpace",
            Self::Delete => "Delete",
            Self::Insert => "Insert",
            Self::Home => "Home",
            Self::End => "End",
            Self::PageUp => "Page_Up",
            Self::PageDown => "Page_Down",
            Self::Left => "Left",
            Self::Up => "Up",
            Self::Right => "Right",
            Self::Down => "Down",
            Self::Minus => "minus",
            Self::Equal => "equal",
            Self::BracketLeft => "bracketleft",
            Self::BracketRight => "bracketright",
            Self::Backslash => "backslash",
            Self::Semicolon => "semicolon",
            Self::Quote => "apostrophe",
            Self::Comma => "comma",
            Self::Period => "period",
            Self::Slash => "slash",
            Self::Backquote => "grave",
        }
    }

    fn keysym(self) -> u32 {
        match self {
            Self::Space => 0x0020,
            Self::Tab => 0xff09,
            Self::Enter => 0xff0d,
            Self::Escape => 0xff1b,
            Self::Backspace => 0xff08,
            Self::Delete => 0xffff,
            Self::Insert => 0xff63,
            Self::Home => 0xff50,
            Self::End => 0xff57,
            Self::PageUp => 0xff55,
            Self::PageDown => 0xff56,
            Self::Left => 0xff51,
            Self::Up => 0xff52,
            Self::Right => 0xff53,
            Self::Down => 0xff54,
            Self::Minus => 0x002d,
            Self::Equal => 0x003d,
            Self::BracketLeft => 0x005b,
            Self::BracketRight => 0x005d,
            Self::Backslash => 0x005c,
            Self::Semicolon => 0x003b,
            Self::Quote => 0x0027,
            Self::Comma => 0x002c,
            Self::Period => 0x002e,
            Self::Slash => 0x002f,
            Self::Backquote => 0x0060,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shortcut {
    ctrl: bool,
    alt: bool,
    shift: bool,
    logo: bool,
    key: Key,
}

impl Shortcut {
    pub fn parse(input: &str) -> Result<Self, String> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err("the shortcut is empty".to_string());
        }

        let mut tokens: Vec<&str> = trimmed
            .split('+')
            .map(str::trim)
            .filter(|token| !token.is_empty())
            .collect();
        // "Ctrl++" means the plus key itself; splitting on '+' loses it.
        if trimmed.ends_with('+') {
            if tokens.is_empty() {
                return Err("include at least one modifier (Ctrl, Alt or Super)".to_string());
            }
            tokens.push("+");
        }
        if tokens.len() < 2 {
            return Err("include at least one modifier (Ctrl, Alt or Super)".to_string());
        }

        let key_token = tokens.pop().expect("checked length").to_ascii_lowercase();
        let key = Self::parse_key(&key_token)?;

        let (mut ctrl, mut alt, mut shift, mut logo) = (false, false, false, false);
        for token in tokens {
            match token.to_ascii_lowercase().as_str() {
                "ctrl" | "control" | "ctl" => {
                    if ctrl {
                        return Err("Ctrl is listed twice".to_string());
                    }
                    ctrl = true;
                }
                "alt" | "option" | "opt" => {
                    if alt {
                        return Err("Alt is listed twice".to_string());
                    }
                    alt = true;
                }
                "shift" => {
                    if shift {
                        return Err("Shift is listed twice".to_string());
                    }
                    shift = true;
                }
                "super" | "meta" | "win" | "windows" | "logo" | "cmd" | "command" => {
                    if logo {
                        return Err("Super is listed twice".to_string());
                    }
                    logo = true;
                }
                other => {
                    return Err(format!("unknown modifier \"{other}\""));
                }
            }
        }

        let modifier = ctrl || alt || logo;
        let function_key = matches!(key, Key::Function(_));
        if !modifier && !(shift && function_key) {
            return Err("use Ctrl, Alt or Super (Shift alone is only allowed with F-keys)".to_string());
        }

        Ok(Self {
            ctrl,
            alt,
            shift,
            logo,
            key,
        })
    }

    fn parse_key(token: &str) -> Result<Key, String> {
        if token.len() == 1 {
            let c = token.chars().next().expect("one character");
            if c.is_ascii_alphanumeric() {
                return Ok(Key::Char(c.to_ascii_lowercase()));
            }
            let named = match c {
                '-' => NamedKey::Minus,
                '=' => NamedKey::Equal,
                '[' => NamedKey::BracketLeft,
                ']' => NamedKey::BracketRight,
                '\\' => NamedKey::Backslash,
                ';' => NamedKey::Semicolon,
                '\'' => NamedKey::Quote,
                ',' => NamedKey::Comma,
                '.' => NamedKey::Period,
                '/' => NamedKey::Slash,
                '`' => NamedKey::Backquote,
                '+' => return Ok(Key::Char('+')),
                _ => return Err(format!("unsupported key \"{token}\"")),
            };
            return Ok(Key::Named(named));
        }

        if let Some(rest) = token.strip_prefix('f') {
            if let Ok(number) = rest.parse::<u8>() {
                if (1..=24).contains(&number) {
                    return Ok(Key::Function(number));
                }
            }
        }

        NamedKey::parse(token)
            .map(Key::Named)
            .ok_or_else(|| format!("unsupported key \"{token}\""))
    }

    pub fn display(&self) -> String {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("Ctrl".to_string());
        }
        if self.alt {
            parts.push("Alt".to_string());
        }
        if self.shift {
            parts.push("Shift".to_string());
        }
        if self.logo {
            parts.push("Super".to_string());
        }
        parts.push(match self.key {
            Key::Char('+') => "Plus".to_string(),
            Key::Char(c) => c.to_ascii_uppercase().to_string(),
            Key::Function(n) => format!("F{n}"),
            Key::Named(named) => named.label().to_string(),
        });
        parts.join("+")
    }

    /// Trigger in the format expected by `org.freedesktop.portal.GlobalShortcuts`.
    pub fn portal_trigger(&self) -> String {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("CTRL".to_string());
        }
        if self.alt {
            parts.push("ALT".to_string());
        }
        if self.shift {
            parts.push("SHIFT".to_string());
        }
        if self.logo {
            parts.push("LOGO".to_string());
        }
        parts.push(match self.key {
            Key::Char('+') => "plus".to_string(),
            Key::Char(c) => c.to_string(),
            Key::Function(n) => format!("F{n}"),
            Key::Named(named) => named.portal_name().to_string(),
        });
        parts.join("+")
    }

    fn x11_modifiers(&self) -> u16 {
        use x11rb::protocol::xproto::ModMask;
        let mut mask = ModMask::default();
        if self.ctrl {
            mask |= ModMask::CONTROL;
        }
        if self.alt {
            mask |= ModMask::M1;
        }
        if self.shift {
            mask |= ModMask::SHIFT;
        }
        if self.logo {
            mask |= ModMask::M4;
        }
        u16::from(mask)
    }

    fn x11_keysym(&self) -> u32 {
        match self.key {
            Key::Char(c) => c as u32,
            Key::Function(n) => 0xffbe + u32::from(n) - 1,
            Key::Named(named) => named.keysym(),
        }
    }
}

impl fmt::Display for Shortcut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.display())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyBackend {
    X11,
    Portal,
}

impl HotkeyBackend {
    pub fn label(self) -> &'static str {
        match self {
            Self::X11 => "X11 key grab",
            Self::Portal => "XDG GlobalShortcuts portal",
        }
    }
}

/// A running hotkey listener. Dropping it unregisters the shortcut.
pub struct HotkeyService {
    pub backend: HotkeyBackend,
    pub shortcut: Shortcut,
    stop: Option<Box<dyn FnOnce() + Send>>,
}

impl Drop for HotkeyService {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            stop();
        }
    }
}

fn forced_backend() -> Option<&'static str> {
    // std::env::var leaks, so use a small match instead.
    match std::env::var("CROSSHAIR_HOTKEYS")
        .ok()
        .as_deref()
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("x11") => Some("x11"),
        Some("portal") => Some("portal"),
        Some("none") => Some("none"),
        _ => None,
    }
}

/// Start listening for the toggle shortcut.
///
/// The returned service keeps working until it is dropped. Failures after
/// startup (portal unavailable, conflicting grab, ...) are reported as
/// [`AppAction::Notice`] on `tx` so they can be shown in the settings window.
pub fn start(input: &str, tx: async_channel::Sender<AppAction>) -> Result<HotkeyService, String> {
    let shortcut = Shortcut::parse(input)?;
    let forced = forced_backend();
    if forced == Some("none") {
        return Err("global hotkeys are disabled by CROSSHAIR_HOTKEYS=none".to_string());
    }

    let use_portal = match forced {
        Some("x11") => false,
        Some("portal") => true,
        _ => std::env::var_os("WAYLAND_DISPLAY").is_some(),
    };

    if use_portal {
        return start_portal(shortcut, tx);
    }
    if std::env::var_os("DISPLAY").is_none() {
        return Err("no display server available for global hotkeys".to_string());
    }
    start_x11(shortcut, tx)
}

// ---------------------------------------------------------------------------
// X11
// ---------------------------------------------------------------------------

fn start_x11(
    shortcut: Shortcut,
    tx: async_channel::Sender<AppAction>,
) -> Result<HotkeyService, String> {
    let (stop_tx, stop_rx) = calloop::channel::channel::<()>();
    let thread_shortcut = shortcut;
    let thread_tx = tx.clone();
    std::thread::Builder::new()
        .name("crosshair-hotkeys-x11".to_string())
        .spawn(move || {
            if let Err(err) = x11::run(thread_shortcut, thread_tx.clone(), stop_rx) {
                let _ = thread_tx.try_send(AppAction::Notice(format!(
                    "Global hotkey unavailable: {err}. You can still use the tray menu or assign a desktop shortcut to \"crosshair-linux --toggle\"."
                )));
            }
        })
        .map_err(|e| format!("spawn hotkey thread: {e}"))?;

    Ok(HotkeyService {
        backend: HotkeyBackend::X11,
        shortcut,
        stop: Some(Box::new(move || {
            let _ = stop_tx.send(());
        })),
    })
}

mod x11 {
    use super::*;
    use calloop::channel::{Channel, Event as ChannelEvent};
    use calloop::generic::Generic;
    use calloop::{EventLoop, Interest, Mode, PostAction};
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{self, ConnectionExt as _, GrabMode, Keycode, Window};
    use x11rb::protocol::Event;
    use x11rb::rust_connection::RustConnection;

    struct Grabs {
        keycode: Keycode,
        modifier_masks: Vec<u16>,
    }

    struct Listener {
        conn: RustConnection,
        root: Window,
        shortcut: Shortcut,
        grabs: Option<Grabs>,
        tx: async_channel::Sender<AppAction>,
        last_toggle: Option<Instant>,
        pressed: bool,
    }

    pub fn run(
        shortcut: Shortcut,
        tx: async_channel::Sender<AppAction>,
        stop_rx: Channel<()>,
    ) -> Result<(), String> {
        let (conn, screen_num) = x11rb::connect(None).map_err(|e| format!("connect to X11: {e}"))?;
        let root = conn.setup().roots[screen_num].root;
        let mut listener = Listener {
            conn,
            root,
            shortcut,
            grabs: None,
            tx,
            last_toggle: None,
            pressed: false,
        };
        listener.bind()?;

        let mut event_loop: EventLoop<Listener> =
            EventLoop::try_new().map_err(|e| format!("create hotkey event loop: {e}"))?;
        let handle = event_loop.handle();
        let signal = event_loop.get_signal();

        let fd = duplicate_fd(listener.conn.stream().as_raw_fd())
            .map_err(|e| format!("duplicate X11 socket: {e}"))?;
        handle
            .insert_source(
                Generic::new(fd, Interest::READ, Mode::Level),
                |_, _, listener: &mut Listener| match listener.handle_events() {
                    Ok(()) => Ok(PostAction::Continue),
                    Err(err) => {
                        eprintln!("crosshair-linux: hotkey event error: {err}");
                        Ok(PostAction::Remove)
                    }
                },
            )
            .map_err(|e| format!("watch X11 socket: {e}"))?;

        let channel_signal = signal.clone();
        handle
            .insert_source(
                stop_rx,
                move |event, _, listener: &mut Listener| {
                    if matches!(event, ChannelEvent::Msg(()) | ChannelEvent::Closed) {
                        listener.unbind();
                        channel_signal.stop();
                    }
                },
            )
            .map_err(|e| format!("watch hotkey shutdown: {e}"))?;

        event_loop
            .run(None, &mut listener, |_| {})
            .map_err(|e| format!("run hotkey event loop: {e}"))?;
        listener.unbind();
        Ok(())
    }

    fn duplicate_fd(fd: i32) -> std::io::Result<OwnedFd> {
        let new_fd = unsafe { libc::dup(fd) };
        if new_fd < 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(unsafe { OwnedFd::from_raw_fd(new_fd) })
        }
    }

    fn keycode_has_keysym(
        mapping: &[u32],
        keysyms_per_keycode: usize,
        keycode: Keycode,
        min_keycode: Keycode,
        keysym: u32,
    ) -> bool {
        let index = keycode.saturating_sub(min_keycode) as usize;
        let start = index * keysyms_per_keycode;
        mapping
            .get(start..start + keysyms_per_keycode)
            .map(|keysyms| keysyms.contains(&keysym))
            .unwrap_or(false)
    }

    impl Listener {
        fn bind(&mut self) -> Result<(), String> {
            self.unbind();
            let grabs = self.resolve_grabs()?;
            self.grabs = Some(grabs);
            self.conn
                .flush()
                .map_err(|e| format!("flush X11 grabs: {e}"))?;
            Ok(())
        }

        fn resolve_grabs(&mut self) -> Result<Grabs, String> {
            let setup = self.conn.setup();
            let min_keycode = setup.min_keycode;
            let max_keycode = setup.max_keycode;
            let count = max_keycode
                .checked_sub(min_keycode)
                .and_then(|n| n.checked_add(1))
                .ok_or_else(|| "invalid keyboard mapping range".to_string())?;
            let mapping_reply = self
                .conn
                .get_keyboard_mapping(min_keycode, count)
                .map_err(|e| format!("read keyboard mapping: {e}"))?
                .reply()
                .map_err(|e| format!("read keyboard mapping: {e}"))?;
            let per = mapping_reply.keysyms_per_keycode as usize;
            let keysym = self.shortcut.x11_keysym();
            let mut keycode = None;
            for index in 0..count as usize {
                let start = index * per;
                let Some(slice) = mapping_reply.keysyms.get(start..start + per) else {
                    break;
                };
                if slice.contains(&keysym) {
                    keycode = Some(min_keycode + index as Keycode);
                    break;
                }
            }
            let keycode = keycode.ok_or_else(|| {
                format!(
                    "the current keyboard layout does not provide the key used by \"{}\"",
                    self.shortcut
                )
            })?;

            let modifier_reply = self
                .conn
                .get_modifier_mapping()
                .map_err(|e| format!("read modifier mapping: {e}"))?
                .reply()
                .map_err(|e| format!("read modifier mapping: {e}"))?;
            let per_modifier = modifier_reply.keycodes_per_modifier() as usize;
            let mut lock_mask = 0u16;
            let mut num_mask = 0u16;
            for (group, keycodes) in modifier_reply
                .keycodes
                .chunks(per_modifier.max(1))
                .enumerate()
                .take(8)
            {
                for &kc in keycodes {
                    if kc == 0 {
                        continue;
                    }
                    if keycode_has_keysym(
                        &mapping_reply.keysyms,
                        per,
                        kc,
                        min_keycode,
                        0xffe5, // Caps_Lock
                    ) {
                        lock_mask = 1 << group;
                    }
                    if keycode_has_keysym(
                        &mapping_reply.keysyms,
                        per,
                        kc,
                        min_keycode,
                        0xff7f, // Num_Lock
                    ) {
                        num_mask = 1 << group;
                    }
                }
            }

            let base = self.shortcut.x11_modifiers();
            let mut masks = vec![base];
            if lock_mask != 0 {
                masks.push(base | lock_mask);
            }
            if num_mask != 0 {
                masks.push(base | num_mask);
                if lock_mask != 0 {
                    masks.push(base | lock_mask | num_mask);
                }
            }
            masks.sort_unstable();
            masks.dedup();

            let mut grabbed = Vec::new();
            let mut conflicts = Vec::new();
            for mask in masks {
                let cookie = match xproto::grab_key(
                    &self.conn,
                    false,
                    self.root,
                    xproto::ModMask::from(mask),
                    keycode,
                    GrabMode::ASYNC,
                    GrabMode::ASYNC,
                ) {
                    Ok(cookie) => cookie,
                    Err(err) => return Err(format!("request key grab: {err}")),
                };
                match cookie.check() {
                    Ok(()) => grabbed.push(mask),
                    Err(err) => conflicts.push(format!("{mask:#x}: {err}")),
                }
            }
            if grabbed.is_empty() {
                return Err(
                    "the shortcut is already grabbed by another application".to_string()
                );
            }
            if !conflicts.is_empty() {
                let _ = self.tx.try_send(AppAction::Notice(format!(
                    "Some modifier variants of {} could not be grabbed (another app may use them).",
                    self.shortcut
                )));
            }

            Ok(Grabs {
                keycode,
                modifier_masks: grabbed,
            })
        }

        fn unbind(&mut self) {
            let Some(grabs) = self.grabs.take() else {
                return;
            };
            for mask in grabs.modifier_masks {
                let _ = xproto::ungrab_key(
                    &self.conn,
                    grabs.keycode,
                    self.root,
                    xproto::ModMask::from(mask),
                );
            }
            let _ = self.conn.flush();
        }

        fn handle_events(&mut self) -> Result<(), String> {
            while let Some(event) = self
                .conn
                .poll_for_event()
                .map_err(|e| format!("read X11 event: {e}"))?
            {
                match event {
                    Event::KeyPress(event) => {
                        let is_ours = self
                            .grabs
                            .as_ref()
                            .map(|grabs| grabs.keycode == event.detail)
                            .unwrap_or(false);
                        if is_ours {
                            self.trigger();
                        }
                    }
                    Event::KeyRelease(event) => {
                        if self
                            .grabs
                            .as_ref()
                            .map(|grabs| grabs.keycode == event.detail)
                            .unwrap_or(false)
                        {
                            self.pressed = false;
                        }
                    }
                    Event::MappingNotify(_) => {
                        // The layout changed; resolve the keycode again.
                        if let Err(err) = self.bind() {
                            let _ = self.tx.try_send(AppAction::Notice(format!(
                                "Could not re-register the global hotkey after a keyboard change: {err}"
                            )));
                        }
                    }
                    _ => {}
                }
            }
            self.conn
                .flush()
                .map_err(|e| format!("flush X11 events: {e}"))?;
            Ok(())
        }

        fn trigger(&mut self) {
            let now = Instant::now();
            if self.pressed {
                return;
            }
            self.pressed = true;
            if self
                .last_toggle
                .map(|last| now.duration_since(last) < TOGGLE_DEBOUNCE)
                .unwrap_or(false)
            {
                return;
            }
            self.last_toggle = Some(now);
            let _ = self.tx.try_send(AppAction::Toggle);
        }
    }
}

// ---------------------------------------------------------------------------
// XDG GlobalShortcuts portal
// ---------------------------------------------------------------------------

fn start_portal(
    shortcut: Shortcut,
    tx: async_channel::Sender<AppAction>,
) -> Result<HotkeyService, String> {
    let (stop_tx, stop_rx) = async_channel::bounded::<()>(1);
    let thread_tx = tx.clone();
    std::thread::Builder::new()
        .name("crosshair-hotkeys-portal".to_string())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(err) => {
                    let _ = thread_tx.try_send(AppAction::Notice(format!(
                        "Could not start the global shortcut service: {err}"
                    )));
                    return;
                }
            };
            if let Err(err) = runtime.block_on(portal::run(shortcut, thread_tx.clone(), stop_rx)) {
                let _ = thread_tx.try_send(AppAction::Notice(format!(
                    "Global shortcut service unavailable: {err}. Use the tray menu or bind \"crosshair-linux --toggle\" as a desktop shortcut."
                )));
            }
        })
        .map_err(|e| format!("spawn global shortcut thread: {e}"))?;

    Ok(HotkeyService {
        backend: HotkeyBackend::Portal,
        shortcut,
        stop: Some(Box::new(move || {
            let _ = stop_tx.try_send(());
        })),
    })
}

mod portal {
    use super::*;
    use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut};
    use ashpd::desktop::CreateSessionOptions;
    use futures_util::StreamExt;

    pub(super) async fn run(
        shortcut: Shortcut,
        tx: async_channel::Sender<AppAction>,
        stop_rx: async_channel::Receiver<()>,
    ) -> Result<(), String> {
        let portal = GlobalShortcuts::new()
            .await
            .map_err(|e| format!("connect to the portal: {e}"))?;
        let mut activated = portal
            .receive_activated()
            .await
            .map_err(|e| format!("listen for shortcut activations: {e}"))?;

        let session = portal
            .create_session(CreateSessionOptions::default())
            .await
            .map_err(|e| format!("create a portal session: {e}"))?;

        let trigger = shortcut.portal_trigger();
        let requested = NewShortcut::new("toggle", "Show or hide the crosshair")
            .preferred_trigger(Some(trigger.as_str()));
        let request = portal
            .bind_shortcuts(&session, &[requested], None, Default::default())
            .await
            .map_err(|e| format!("bind the shortcut: {e}"))?;
        let response = request
            .response()
            .map_err(|e| format!("bind the shortcut: {e}"))?;
        let bound = response
            .shortcuts()
            .iter()
            .find(|info| info.id() == "toggle");
        match bound {
            Some(info) => {
                let description = info.trigger_description();
                if !description.is_empty() && description != trigger {
                    eprintln!(
                        "crosshair-linux: portal bound \"{description}\" instead of \"{trigger}\""
                    );
                }
            }
            None => {
                return Err("the desktop portal did not register the shortcut (it may not be supported on this desktop)".to_string());
            }
        }

        loop {
            tokio::select! {
                _ = stop_rx.recv() => break,
                activation = activated.next() => {
                    match activation {
                        Some(activation) => {
                            if activation.shortcut_id() == "toggle" {
                                let _ = tx.try_send(AppAction::Toggle);
                            }
                        }
                        None => break,
                    }
                }
            }
        }

        let _ = session.close().await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_shortcuts() {
        let shortcut = Shortcut::parse("Ctrl+Shift+X").unwrap();
        assert_eq!(shortcut.display(), "Ctrl+Shift+X");
        assert_eq!(shortcut.portal_trigger(), "CTRL+SHIFT+x");
        assert_eq!(shortcut.x11_modifiers(), 0b101);

        let shortcut = Shortcut::parse("super + alt + f5").unwrap();
        assert_eq!(shortcut.display(), "Alt+Super+F5");
        assert_eq!(shortcut.portal_trigger(), "ALT+LOGO+F5");
        assert_eq!(shortcut.x11_keysym(), 0xffc2);

        let shortcut = Shortcut::parse("Ctrl+-").unwrap();
        assert_eq!(shortcut.display(), "Ctrl+Minus");
        assert_eq!(shortcut.portal_trigger(), "CTRL+minus");
        assert_eq!(shortcut.x11_keysym(), 0x2d);
    }

    #[test]
    fn parses_plus_key() {
        let shortcut = Shortcut::parse("Ctrl++").unwrap();
        assert_eq!(shortcut.display(), "Ctrl+Plus");
        assert_eq!(shortcut.portal_trigger(), "CTRL+plus");
        assert_eq!(shortcut.x11_keysym(), '+' as u32);
    }

    #[test]
    fn rejects_invalid_shortcuts() {
        assert!(Shortcut::parse("").is_err());
        assert!(Shortcut::parse("X").is_err());
        assert!(Shortcut::parse("Shift+X").is_err());
        assert!(Shortcut::parse("Ctrl+Ctrl+X").is_err());
        assert!(Shortcut::parse("Ctrl+Unknown").is_err());
        assert!(Shortcut::parse("Ctrl+F25").is_err());
        assert!(Shortcut::parse("Hyper+X").is_err());
        assert!(Shortcut::parse("Shift+F2").is_ok());
    }
}
