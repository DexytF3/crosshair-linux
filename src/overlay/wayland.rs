//! Native Wayland overlay backend built on `zwlr_layer_shell_v1`.
//!
//! One small overlay layer surface is created per target output, positioned
//! with anchors and margins. Surfaces use `Layer::Overlay`, never take keyboard
//! focus, and install an empty `wl_region` as their input region. Pixels come
//! from a `wl_shm` pool; nothing is redrawn unless the configuration or an
//! output changes.

use crate::model::{DisplayInfo, PositionMode};
use crate::overlay::{local_origin, target_display, OverlayCommand, OverlaySpec};
use crate::render::{self, RenderOptions, Source};
use calloop::channel::{Channel, Event as ChannelEvent};
use calloop::EventLoop;
use smithay_client_toolkit::compositor::{CompositorHandler, CompositorState, Region};
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::registry_handlers;
use smithay_client_toolkit::shell::wlr_layer::{
    Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
    LayerSurfaceConfigure,
};
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shm::slot::SlotPool;
use smithay_client_toolkit::shm::{Shm, ShmHandler};
use smithay_client_toolkit::{delegate_registry, delegate_dispatch2};
use wayland_client::globals::registry_queue_init;
use wayland_client::protocol::{wl_output, wl_shm, wl_surface};
use wayland_client::{Connection, QueueHandle};

pub fn available() -> bool {
    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
        return false;
    }
    let Ok(conn) = Connection::connect_to_env() else {
        return false;
    };
    let Ok((globals, _queue)) = registry_queue_init::<DisplayProbe>(&conn) else {
        return false;
    };
    let mut has_compositor = false;
    let mut has_shm = false;
    let mut has_layer_shell = false;
    globals.contents().with_list(|list| {
        for global in list {
            match global.interface.as_str() {
                "wl_compositor" => has_compositor = true,
                "wl_shm" => has_shm = true,
                "zwlr_layer_shell_v1" => has_layer_shell = true,
                _ => {}
            }
        }
    });
    has_compositor && has_shm && has_layer_shell
}

fn logical_size(info: &smithay_client_toolkit::output::OutputInfo, scale: i32) -> (i32, i32) {
    if let Some(size) = info.logical_size {
        return size;
    }
    if let Some(mode) = info.modes.iter().find(|mode| mode.current) {
        let scale = scale.max(1);
        return (mode.dimensions.0 / scale, mode.dimensions.1 / scale);
    }
    (1920, 1080)
}

fn display_from_output(
    info: &smithay_client_toolkit::output::OutputInfo,
    index: usize,
    primary: bool,
) -> DisplayInfo {
    let scale = info.scale_factor.max(1);
    let (width, height) = logical_size(info, scale);
    let name = info
        .name
        .clone()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| {
            if info.model.trim().is_empty() {
                format!("Output {}", index + 1)
            } else {
                info.model.clone()
            }
        });
    let (x, y) = info
        .logical_position
        .or(Some(info.location))
        .unwrap_or((0, 0));
    DisplayInfo {
        id: format!("wayland:{}", info.id),
        name,
        x,
        y,
        width: u32::try_from(width).unwrap_or(1920),
        height: u32::try_from(height).unwrap_or(1080),
        primary,
        scale,
    }
}

fn collect_displays(output_state: &OutputState) -> Vec<DisplayInfo> {
    let mut outputs: Vec<_> = output_state
        .outputs()
        .filter_map(|output| output_state.info(&output))
        .collect();
    outputs.sort_by_key(|info| info.logical_position.or(Some(info.location)).unwrap_or((0, 0)));
    let has_zero_origin = outputs
        .iter()
        .any(|info| info.logical_position.or(Some(info.location)) == Some((0, 0)));
    outputs
        .iter()
        .enumerate()
        .map(|(index, info)| {
            let primary = if has_zero_origin {
                info.logical_position.or(Some(info.location)) == Some((0, 0))
            } else {
                index == 0
            };
            display_from_output(info, index, primary)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Display enumeration (used by the settings window)
// ---------------------------------------------------------------------------

struct DisplayProbe {
    registry_state: RegistryState,
    output_state: OutputState,
}

impl OutputHandler for DisplayProbe {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl ProvidesRegistryState for DisplayProbe {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState];
}

delegate_registry!(DisplayProbe);
delegate_dispatch2!(DisplayProbe);

pub fn list_displays() -> Result<Vec<DisplayInfo>, String> {
    let conn = Connection::connect_to_env().map_err(|e| format!("connect to Wayland: {e}"))?;
    let (globals, mut queue) =
        registry_queue_init(&conn).map_err(|e| format!("initialize Wayland registry: {e}"))?;
    let qh = queue.handle();
    let mut probe = DisplayProbe {
        registry_state: RegistryState::new(&globals),
        output_state: OutputState::new(&globals, &qh),
    };
    queue
        .roundtrip(&mut probe)
        .map_err(|e| format!("read Wayland outputs: {e}"))?;
    queue
        .roundtrip(&mut probe)
        .map_err(|e| format!("read Wayland output details: {e}"))?;
    Ok(collect_displays(&probe.output_state))
}

// ---------------------------------------------------------------------------
// Overlay
// ---------------------------------------------------------------------------

struct SurfaceEntry {
    output_name: String,
    layer: LayerSurface,
    pool: SlotPool,
    width: u32,
    scale: u32,
    configured: bool,
}

struct WaylandOverlay {
    conn: Connection,
    registry_state: RegistryState,
    compositor: CompositorState,
    layer_shell: LayerShell,
    shm: Shm,
    output_state: OutputState,
    qh: QueueHandle<Self>,
    surfaces: Vec<SurfaceEntry>,
    spec: OverlaySpec,
    visible: bool,
    ready: bool,
}

impl WaylandOverlay {
    fn apply(&mut self) -> Result<(), String> {
        // Dropping old surfaces destroys their pools and buffers. Updates are
        // infrequent, so recreating is simpler and keeps the shm pool from
        // accumulating dead slots.
        self.surfaces.clear();
        if !self.visible {
            self.conn
                .flush()
                .map_err(|e| format!("flush Wayland requests: {e}"))?;
            return Ok(());
        }

        let size = self.spec.style.clamped_size().max(1);
        let displays = collect_displays(&self.output_state);
        let target_id = if self.spec.show_all {
            None
        } else {
            target_display(&displays, &self.spec.position).map(|d| d.id.clone())
        };

        let outputs = self.output_state.outputs().collect::<Vec<_>>();
        for (index, output) in outputs.iter().enumerate() {
            let Some(info) = self.output_state.info(output) else {
                continue;
            };
            let display = display_from_output(&info, index, false);

            if let Some(target) = target_id.as_deref() {
                if display.id != target {
                    continue;
                }
            }

            let scale = u32::try_from(info.scale_factor.max(1)).unwrap_or(1);
            let surface = self.compositor.create_surface(&self.qh);
            let layer = self.layer_shell.create_layer_surface(
                &self.qh,
                surface,
                Layer::Overlay,
                Some("crosshair-linux"),
                Some(output),
            );
            layer.set_anchor(Anchor::TOP | Anchor::LEFT);
            layer.set_keyboard_interactivity(KeyboardInteractivity::None);
            layer.set_exclusive_zone(-1);
            layer.set_size(size, size);

            let (x, y) = if self.spec.position.mode == PositionMode::Follow {
                // Follow mode is unavailable natively; fall back to center.
                let mut position = self.spec.position.clone();
                position.mode = PositionMode::Center;
                position.x = 0;
                position.y = 0;
                local_origin(&display, size, &position)
            } else {
                local_origin(&display, size, &self.spec.position)
            };
            layer.set_margin(y, 0, 0, x);

            match Region::new(&self.compositor) {
                Ok(region) => layer.set_input_region(Some(region.wl_region())),
                Err(_) => layer.set_input_region(None),
            }

            let buffer_size = size * scale;
            let pool_len = (buffer_size as usize)
                .saturating_mul(buffer_size as usize)
                .saturating_mul(4);
            let pool = SlotPool::new(pool_len.max(4), &self.shm)
                .map_err(|e| format!("create Wayland shm pool: {e}"))?;

            layer.commit();
            self.surfaces.push(SurfaceEntry {
                output_name: display.name,
                layer,
                pool,
                width: size,
                scale,
                configured: false,
            });
        }

        self.conn
            .flush()
            .map_err(|e| format!("flush Wayland requests: {e}"))?;
        Ok(())
    }

    fn draw(&mut self, index: usize) -> Result<(), String> {
        let Some(entry) = self.surfaces.get(index) else {
            return Ok(());
        };
        let surface = entry.layer.wl_surface().clone();
        let width = entry.width;
        let scale = entry.scale.max(1);
        let buffer_width = width * scale;
        let buffer_height = width * scale;
        let stride = (buffer_width * 4) as i32;
        let _ = &entry.output_name;

        let pixmap = render::render(
            &Source::from_config(&self.spec.style.crosshair),
            &RenderOptions::from_style(&self.spec.style, buffer_width),
        );
        let pixels = render::pixmap_to_bgra(&pixmap);

        let entry = &mut self.surfaces[index];
        let (buffer, canvas) = entry
            .pool
            .create_buffer(
                buffer_width as i32,
                buffer_height as i32,
                stride,
                wl_shm::Format::Argb8888,
            )
            .map_err(|e| format!("allocate Wayland buffer: {e}"))?;
        canvas[..pixels.len()].copy_from_slice(&pixels);
        let _ = surface.set_buffer_scale(scale as i32);
        surface.damage_buffer(0, 0, buffer_width as i32, buffer_height as i32);
        buffer
            .attach_to(&surface)
            .map_err(|e| format!("attach Wayland buffer: {e}"))?;
        entry.layer.commit();
        entry.configured = true;
        self.conn
            .flush()
            .map_err(|e| format!("flush Wayland frame: {e}"))?;
        Ok(())
    }

    fn shutdown(&mut self) {
        self.surfaces.clear();
        let _ = self.conn.flush();
    }
}

impl CompositorHandler for WaylandOverlay {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: i32,
    ) {
    }

    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: u32,
    ) {
    }

    fn surface_enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for WaylandOverlay {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {
        if self.ready {
            if let Err(err) = self.apply() {
                eprintln!("crosshair-linux: Wayland output change failed: {err}");
            }
        }
    }

    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {
        if self.ready {
            if let Err(err) = self.apply() {
                eprintln!("crosshair-linux: Wayland output update failed: {err}");
            }
        }
    }

    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {
        if self.ready {
            if let Err(err) = self.apply() {
                eprintln!("crosshair-linux: Wayland output removal failed: {err}");
            }
        }
    }
}

impl LayerShellHandler for WaylandOverlay {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface) {
        self.surfaces.clear();
    }

    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        layer: &LayerSurface,
        _: LayerSurfaceConfigure,
        _: u32,
    ) {
        let Some(index) = self.surfaces.iter().position(|entry| &entry.layer == layer) else {
            return;
        };
        if self.surfaces[index].configured {
            return;
        }
        if let Err(err) = self.draw(index) {
            eprintln!("crosshair-linux: Wayland overlay draw failed: {err}");
        }
    }
}

impl ShmHandler for WaylandOverlay {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl ProvidesRegistryState for WaylandOverlay {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState];
}

delegate_registry!(WaylandOverlay);
delegate_dispatch2!(WaylandOverlay);

pub fn run(rx: Channel<OverlayCommand>, initial: OverlaySpec, visible: bool) -> Result<(), String> {
    let conn = Connection::connect_to_env().map_err(|e| format!("connect to Wayland: {e}"))?;
    let (globals, mut queue) =
        registry_queue_init(&conn).map_err(|e| format!("initialize Wayland registry: {e}"))?;
    let qh = queue.handle();

    let compositor = CompositorState::bind(&globals, &qh)
        .map_err(|e| format!("wl_compositor is unavailable: {e}"))?;
    let layer_shell = LayerShell::bind(&globals, &qh)
        .map_err(|e| format!("zwlr_layer_shell_v1 is unavailable: {e}"))?;
    let shm = Shm::bind(&globals, &qh).map_err(|e| format!("wl_shm is unavailable: {e}"))?;
    let registry_state = RegistryState::new(&globals);
    let output_state = OutputState::new(&globals, &qh);

    let mut state = WaylandOverlay {
        conn: conn.clone(),
        registry_state,
        compositor,
        layer_shell,
        shm,
        output_state,
        qh,
        surfaces: Vec::new(),
        spec: initial,
        visible,
        ready: false,
    };

    queue
        .roundtrip(&mut state)
        .map_err(|e| format!("read Wayland outputs: {e}"))?;
    state.apply()?;
    state.ready = true;

    let mut event_loop: EventLoop<WaylandOverlay> =
        EventLoop::try_new().map_err(|e| format!("create Wayland event loop: {e}"))?;
    let handle = event_loop.handle();
    let signal = event_loop.get_signal();

    smithay_client_toolkit::reexports::calloop_wayland_source::WaylandSource::new(conn, queue)
        .insert(handle.clone())
        .map_err(|e| format!("watch Wayland socket: {e}"))?;

    let channel_signal = signal.clone();
    handle
        .insert_source(
            rx,
            move |event, _, state: &mut WaylandOverlay| match event {
                ChannelEvent::Msg(OverlayCommand::Update { spec, visible }) => {
                    state.spec = spec;
                    state.visible = visible;
                    if let Err(err) = state.apply() {
                        eprintln!("crosshair-linux: Wayland overlay update failed: {err}");
                    }
                }
                ChannelEvent::Msg(OverlayCommand::Shutdown) => {
                    state.shutdown();
                    channel_signal.stop();
                }
                ChannelEvent::Closed => channel_signal.stop(),
            },
        )
        .map_err(|e| format!("watch overlay commands: {e}"))?;

    event_loop
        .run(None, &mut state, |_| {})
        .map_err(|e| format!("run Wayland event loop: {e}"))?;
    Ok(())
}
