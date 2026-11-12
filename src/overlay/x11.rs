//! Native X11 / XWayland overlay backend.
//!
//! The overlay uses override-redirect ARGB windows and removes the input shape
//! so all pointer events pass through to the window underneath. It renders
//! only when the settings change (or on Expose) and follows the pointer using
//! X11 motion events when follow mode is enabled.

use crate::model::{DisplayInfo, PositionMode};
use crate::overlay::{
    followed_origin, local_origin, target_display, OverlayCommand, OverlaySpec,
};
use crate::render::{self, RenderOptions, Source};
use calloop::channel::{Channel, Event as ChannelEvent};
use calloop::generic::Generic;
use calloop::{EventLoop, Interest, Mode, PostAction};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use x11rb::connection::{Connection, RequestConnection};
use x11rb::protocol::xproto::ConnectionExt as _;
use x11rb::protocol::{randr, shape, xproto};
use x11rb::rust_connection::RustConnection;

pub fn available() -> bool {
    if std::env::var_os("DISPLAY").is_none() {
        return false;
    }
    match x11rb::connect(None) {
        Ok((conn, _)) => matches!(
            conn.extension_information(shape::X11_EXTENSION_NAME),
            Ok(Some(_))
        ),
        Err(_) => false,
    }
}

fn atom_name(conn: &RustConnection, atom: xproto::Atom) -> Option<String> {
    if atom == x11rb::NONE {
        return None;
    }
    let reply = conn.get_atom_name(atom).ok()?.reply().ok()?;
    let name = String::from_utf8_lossy(&reply.name).trim().to_string();
    (!name.is_empty()).then_some(name)
}

pub fn list_displays() -> Result<Vec<DisplayInfo>, String> {
    let (conn, screen_num) =
        x11rb::connect(None).map_err(|e| format!("connect to X11: {e}"))?;
    let screen = &conn.setup().roots[screen_num];
    let root = screen.root;
    let mut displays = Vec::new();

    if let Ok(reply) = randr::get_monitors(&conn, root, true)
        .map_err(|e| e.to_string())
        .and_then(|cookie| cookie.reply().map_err(|e| e.to_string()))
    {
        for (index, monitor) in reply.monitors.iter().enumerate() {
            let name = atom_name(&conn, monitor.name)
                .unwrap_or_else(|| format!("Display {}", index + 1));
            let mut id = name.clone();
            if displays.iter().any(|d: &DisplayInfo| d.id == id) {
                id = format!("{name} ({index})");
            }
            displays.push(DisplayInfo {
                id,
                name,
                x: i32::from(monitor.x),
                y: i32::from(monitor.y),
                width: u32::from(monitor.width),
                height: u32::from(monitor.height),
                primary: monitor.primary,
                scale: 1,
            });
        }
    }

    if displays.is_empty() {
        displays.push(DisplayInfo {
            id: "x11-root".to_string(),
            name: "Display".to_string(),
            x: 0,
            y: 0,
            width: u32::from(screen.width_in_pixels),
            height: u32::from(screen.height_in_pixels),
            primary: true,
            scale: 1,
        });
    }

    if !displays.iter().any(|d| d.primary) {
        if let Some(first) = displays.first_mut() {
            first.primary = true;
        }
    }
    Ok(displays)
}

fn find_argb_visual(
    conn: &RustConnection,
    screen_num: usize,
) -> Result<(u8, xproto::Visualid), String> {
    let screen = &conn.setup().roots[screen_num];
    let depth = screen
        .allowed_depths
        .iter()
        .find(|d| d.depth == 32)
        .ok_or_else(|| "the X server does not offer a 32-bit ARGB visual".to_string())?;
    let visual = depth
        .visuals
        .iter()
        .find(|v| v.class == xproto::VisualClass::TRUE_COLOR)
        .or_else(|| depth.visuals.first())
        .ok_or_else(|| "the 32-bit visual has no visuals attached".to_string())?;
    Ok((depth.depth, visual.visual_id))
}

struct OverlayWindow {
    display: String,
    window: xproto::Window,
    gc: xproto::Gcontext,
    width: u16,
    height: u16,
    mapped: bool,
    dirty: bool,
}

struct X11Overlay {
    conn: RustConnection,
    root: xproto::Window,
    depth: u8,
    visual: xproto::Visualid,
    colormap: xproto::Colormap,
    windows: Vec<OverlayWindow>,
    displays: Vec<DisplayInfo>,
    spec: OverlaySpec,
    visible: bool,
    image: Vec<u8>,
    image_size: u32,
}

impl X11Overlay {
    fn new(conn: RustConnection, screen_num: usize) -> Result<Self, String> {
        let screen = conn.setup().roots[screen_num].clone();
        let (depth, visual) = find_argb_visual(&conn, screen_num)?;
        let colormap = conn
            .generate_id()
            .map_err(|e| format!("allocate colormap id: {e}"))?;
        xproto::create_colormap(
            &conn,
            xproto::ColormapAlloc::NONE,
            colormap,
            screen.root,
            visual,
        )
        .map_err(|e| format!("create colormap: {e}"))?;

        // Receive pointer motion events at the root so follow mode can move the
        // overlay without polling. Opaque winit-like windows that select motion
        // events themselves will keep those events, which is expected.
        let _ = xproto::change_window_attributes(
            &conn,
            screen.root,
            &xproto::ChangeWindowAttributesAux::new()
                .event_mask(xproto::EventMask::POINTER_MOTION),
        );

        Ok(Self {
            conn,
            root: screen.root,
            depth,
            visual,
            colormap,
            windows: Vec::new(),
            displays: Vec::new(),
            spec: OverlaySpec::new(
                crate::model::Style::default(),
                crate::model::Position::default(),
                false,
            ),
            visible: false,
            image: Vec::new(),
            image_size: 0,
        })
    }

    fn render_image(&mut self) {
        let size = self.spec.style.clamped_size();
        let pixmap = render::render(
            &Source::from_config(&self.spec.style.crosshair),
            &RenderOptions::from_style(&self.spec.style, size),
        );
        self.image = if self
            .conn
            .setup()
            .image_byte_order
            == xproto::ImageOrder::LSB_FIRST
        {
            render::pixmap_to_bgra(&pixmap)
        } else {
            render::pixmap_to_argb_network(&pixmap)
        };
        self.image_size = size;
        for window in &mut self.windows {
            window.dirty = true;
        }
    }

    fn create_window(&mut self, display: &str, width: u16, height: u16) -> Result<OverlayWindow, String> {
        let window = self
            .conn
            .generate_id()
            .map_err(|e| format!("allocate window id: {e}"))?;
        let gc = self
            .conn
            .generate_id()
            .map_err(|e| format!("allocate gc id: {e}"))?;
        let aux = xproto::CreateWindowAux::new()
            .background_pixel(0u32)
            .border_pixel(0u32)
            .override_redirect(1u32)
            .event_mask(xproto::EventMask::EXPOSURE)
            .colormap(self.colormap);
        xproto::create_window(
            &self.conn,
            self.depth,
            window,
            self.root,
            0,
            0,
            width,
            height,
            0,
            xproto::WindowClass::INPUT_OUTPUT,
            self.visual,
            &aux,
        )
        .map_err(|e| format!("create overlay window: {e}"))?;
        xproto::create_gc(
            &self.conn,
            gc,
            window,
            &xproto::CreateGCAux::new(),
        )
        .map_err(|e| format!("create overlay gc: {e}"))?;

        // Empty input shape: the window is visually present but completely
        // click-through.
        shape::rectangles(
            &self.conn,
            shape::SO::SET,
            shape::SK::INPUT,
            xproto::ClipOrdering::UNSORTED,
            window,
            0,
            0,
            &[],
        )
        .map_err(|e| format!("set empty input shape: {e}"))?;

        // Helpful for debugging with xprop; override-redirect windows never
        // appear in the taskbar.
        let title = b"Crosshair Linux overlay";
        let _ = xproto::change_property(
            &self.conn,
            xproto::PropMode::REPLACE,
            window,
            xproto::AtomEnum::WM_NAME,
            xproto::AtomEnum::STRING,
            8,
            title.len() as u32,
            title,
        );

        Ok(OverlayWindow {
            display: display.to_string(),
            window,
            gc,
            width,
            height,
            mapped: false,
            dirty: true,
        })
    }

    fn destroy_window(&self, window: &OverlayWindow) {
        let _ = xproto::free_gc(&self.conn, window.gc);
        let _ = xproto::destroy_window(&self.conn, window.window);
    }

    fn put_image(&self, window: &OverlayWindow) -> Result<(), String> {
        if window.width == 0 || window.height == 0 {
            return Ok(());
        }
        xproto::put_image(
            &self.conn,
            xproto::ImageFormat::Z_PIXMAP,
            window.window,
            window.gc,
            window.width,
            window.height,
            0,
            0,
            0,
            self.depth,
            &self.image,
        )
        .map(|_| ())
        .map_err(|e| format!("upload overlay pixels: {e}"))
    }

    fn apply(&mut self) -> Result<(), String> {
        self.displays = list_displays()?;
        let displays = self.displays.clone();
        let size = self.spec.style.clamped_size().max(1) as u16;
        if self.image.len() != (size as usize) * (size as usize) * 4 || self.image_size != u32::from(size) {
            self.render_image();
        }

        let follow = self.spec.position.mode == PositionMode::Follow;
        let targets: Vec<DisplayInfo> = if !self.visible {
            Vec::new()
        } else if follow {
            target_display(&displays, &self.spec.position)
                .cloned()
                .into_iter()
                .collect()
        } else if self.spec.show_all {
            displays.clone()
        } else {
            target_display(&displays, &self.spec.position)
                .cloned()
                .into_iter()
                .collect()
        };

        let pointer = if follow && self.visible {
            self.conn
                .query_pointer(self.root)
                .ok()
                .and_then(|cookie| cookie.reply().ok())
                .map(|reply| (i32::from(reply.root_x), i32::from(reply.root_y)))
        } else {
            None
        };

        let mut keep = Vec::with_capacity(targets.len());
        for display in &targets {
            let (x, y) = if follow {
                let p = pointer.unwrap_or((display.x, display.y));
                followed_origin(display, u32::from(size), &self.spec.position, p)
            } else {
                let (lx, ly) = local_origin(display, u32::from(size), &self.spec.position);
                (display.x + lx, display.y + ly)
            };

            let existing = self
                .windows
                .iter()
                .position(|w| w.display == display.id);
            let index = if let Some(index) = existing {
                index
            } else {
                let window = self.create_window(&display.id, size, size)?;
                self.windows.push(window);
                self.windows.len() - 1
            };

            {
                let window = &mut self.windows[index];
                let mut aux = xproto::ConfigureWindowAux::new().x(x).y(y);
                if window.width != size || window.height != size {
                    aux = aux.width(u32::from(size)).height(u32::from(size));
                    window.width = size;
                    window.height = size;
                    window.dirty = true;
                }
                xproto::configure_window(&self.conn, window.window, &aux)
                    .map_err(|e| format!("move overlay window: {e}"))?;
            }
            if self.windows[index].dirty {
                self.put_image(&self.windows[index])?;
                self.windows[index].dirty = false;
            }
            {
                let window = &mut self.windows[index];
                if !window.mapped {
                    xproto::map_window(&self.conn, window.window)
                        .map_err(|e| format!("map overlay window: {e}"))?;
                    xproto::configure_window(
                        &self.conn,
                        window.window,
                        &xproto::ConfigureWindowAux::new().stack_mode(xproto::StackMode::ABOVE),
                    )
                    .map_err(|e| format!("raise overlay window: {e}"))?;
                    window.mapped = true;
                }
            }
            keep.push(index);
        }

        // Unmap and destroy stale windows (display removed, overlay hidden or a
        // different display selected).
        let mut kept = Vec::new();
        for (index, mut window) in std::mem::take(&mut self.windows).into_iter().enumerate() {
            if keep.contains(&index) {
                kept.push(window);
            } else {
                if window.mapped {
                    let _ = xproto::unmap_window(&self.conn, window.window);
                    window.mapped = false;
                }
                // Keep a pooled window for displays that are only temporarily
                // deselected; destroy windows for displays that vanished.
                if displays.iter().any(|d| d.id == window.display) {
                    kept.push(window);
                } else {
                    self.destroy_window(&window);
                }
            }
        }
        self.windows = kept;
        self.conn
            .flush()
            .map_err(|e| format!("flush X11 requests: {e}"))?;
        Ok(())
    }

    fn follow_to(&mut self, x: i32, y: i32) -> Result<(), String> {
        if !self.visible || self.spec.position.mode != PositionMode::Follow {
            return Ok(());
        }
        let displays = self.displays.clone();
        let Some(display) = target_display(&displays, &self.spec.position).cloned() else {
            return Ok(());
        };
        let size = self.spec.style.clamped_size();
        let (wx, wy) = followed_origin(&display, size, &self.spec.position, (x, y));
        for window in &self.windows {
            xproto::configure_window(
                &self.conn,
                window.window,
                &xproto::ConfigureWindowAux::new().x(wx).y(wy),
            )
            .map_err(|e| format!("move follow overlay: {e}"))?;
        }
        self.conn
            .flush()
            .map_err(|e| format!("flush X11 requests: {e}"))?;
        Ok(())
    }

    fn handle_events(&mut self) -> Result<(), String> {
        while let Some(event) = self
            .conn
            .poll_for_event()
            .map_err(|e| format!("read X11 event: {e}"))?
        {
            match event {
                x11rb::protocol::Event::Expose(event) => {
                    if event.count == 0 {
                        for window in &mut self.windows {
                            if window.window == event.window {
                                window.dirty = true;
                            }
                        }
                    }
                }
                x11rb::protocol::Event::MotionNotify(event) => {
                    self.follow_to(i32::from(event.root_x), i32::from(event.root_y))?;
                }
                _ => {}
            }
        }
        for index in 0..self.windows.len() {
            if self.windows[index].dirty {
                self.put_image(&self.windows[index])?;
                self.windows[index].dirty = false;
            }
        }
        self.conn
            .flush()
            .map_err(|e| format!("flush X11 requests: {e}"))?;
        Ok(())
    }

    fn shutdown(&mut self) {
        for window in &self.windows {
            let _ = xproto::unmap_window(&self.conn, window.window);
            self.destroy_window(window);
        }
        self.windows.clear();
        let _ = xproto::free_colormap(&self.conn, self.colormap);
        let _ = self.conn.flush();
    }
}

impl Drop for X11Overlay {
    fn drop(&mut self) {
        if !self.windows.is_empty() {
            self.shutdown();
        }
    }
}

fn duplicate_fd(fd: i32) -> std::io::Result<OwnedFd> {
    let new_fd = unsafe { libc::dup(fd) };
    if new_fd < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(unsafe { OwnedFd::from_raw_fd(new_fd) })
    }
}

pub fn run(rx: Channel<OverlayCommand>, initial: OverlaySpec, visible: bool) -> Result<(), String> {
    let (conn, screen_num) = x11rb::connect(None).map_err(|e| format!("connect to X11: {e}"))?;
    let mut overlay = X11Overlay::new(conn, screen_num)?;
    overlay.spec = initial;
    overlay.visible = visible;
    overlay.render_image();
    overlay.apply()?;

    let mut event_loop: EventLoop<X11Overlay> =
        EventLoop::try_new().map_err(|e| format!("create X11 event loop: {e}"))?;
    let handle = event_loop.handle();
    let signal = event_loop.get_signal();

    let x_fd = duplicate_fd(overlay.conn.stream().as_raw_fd())
        .map_err(|e| format!("duplicate X11 socket: {e}"))?;
    handle
        .insert_source(
            Generic::new(x_fd, Interest::READ, Mode::Level),
            |_, _, overlay: &mut X11Overlay| match overlay.handle_events() {
                Ok(()) => Ok(PostAction::Continue),
                Err(err) => {
                    eprintln!("crosshair-linux: X11 overlay event error: {err}");
                    Ok(PostAction::Remove)
                }
            },
        )
        .map_err(|e| format!("watch X11 socket: {e}"))?;

    let channel_signal = signal.clone();
    handle
        .insert_source(
            rx,
            move |event, _, overlay: &mut X11Overlay| match event {
                ChannelEvent::Msg(OverlayCommand::Update { spec, visible }) => {
                    overlay.spec = spec;
                    overlay.visible = visible;
                    overlay.render_image();
                    if let Err(err) = overlay.apply() {
                        eprintln!("crosshair-linux: X11 overlay update failed: {err}");
                    }
                }
                ChannelEvent::Msg(OverlayCommand::Shutdown) => {
                    overlay.shutdown();
                    channel_signal.stop();
                }
                ChannelEvent::Closed => channel_signal.stop(),
            },
        )
        .map_err(|e| format!("watch overlay commands: {e}"))?;

    event_loop
        .run(None, &mut overlay, |_| {})
        .map_err(|e| format!("run X11 event loop: {e}"))?;
    Ok(())
}
