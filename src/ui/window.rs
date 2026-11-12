//! Main settings window, built with standard GTK4 widgets and a custom CSS
//! theme. The layout is intentionally two-column so it feels like a native
//! desktop preferences dialog rather than a web page.

use crate::app::Controller;
use crate::assets;
use crate::model::{
    DisplayInfo, PositionMode, Theme, APP_NAME, APP_VERSION,
};
use crate::overlay::Backend;
use crate::ui::designer;
use crate::ui::gallery::{self, ThumbCache};
use gtk4::prelude::*;
use gtk4::{gdk, gio, glib};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

pub struct Ui {
    pub window: gtk4::ApplicationWindow,
    pub sync: Rc<Cell<bool>>,
    pub thumb_cache: ThumbCache,

    pub status_pill: gtk4::Label,
    pub enable_switch: gtk4::Switch,

    pub preview_picture: gtk4::Picture,
    pub preview_caption: gtk4::Label,

    pub size_scale: gtk4::Scale,
    pub size_value: gtk4::Label,
    pub hue_scale: gtk4::Scale,
    pub hue_value: gtk4::Label,
    pub rotation_scale: gtk4::Scale,
    pub rotation_value: gtk4::Label,
    pub opacity_scale: gtk4::Scale,
    pub opacity_value: gtk4::Label,
    pub reset_style: gtk4::Button,

    pub builtin_button: gtk4::ToggleButton,
    pub custom_button: gtk4::ToggleButton,
    pub show_custom: Cell<bool>,
    pub crosshair_actions: gtk4::Box,
    pub tile_flow: gtk4::FlowBox,
    pub tiles: RefCell<Vec<(String, gtk4::Button)>>,
    pub tiles_theme: Cell<Option<Theme>>,
    pub tiles_dirty: Cell<bool>,
    pub customs: RefCell<Vec<(String, String)>>,

    pub display_box: gtk4::Box,
    pub display_buttons: RefCell<Vec<(String, gtk4::Button)>>,
    pub display_ids: RefCell<Vec<String>>,
    pub display_note: gtk4::Label,
    pub all_displays_switch: gtk4::Switch,

    pub mode_buttons: RefCell<Vec<(PositionMode, gtk4::ToggleButton)>>,
    pub offset_x: gtk4::SpinButton,
    pub offset_y: gtk4::SpinButton,
    pub offset_x_label: gtk4::Label,
    pub offset_y_label: gtk4::Label,
    pub position_hint: gtk4::Label,

    pub theme_dropdown: gtk4::DropDown,
    pub tray_switch: gtk4::Switch,
    pub hotkey_entry: gtk4::Entry,
    pub hotkey_apply: gtk4::Button,
    pub hotkey_status: gtk4::Label,
    pub backend_label: gtk4::Label,
    pub backend_hint: gtk4::Label,

    pub notice_revealer: gtk4::Revealer,
    pub notice_label: gtk4::Label,
    pub notice_source: RefCell<Option<glib::SourceId>>,
}

fn field_label(text: &str) -> gtk4::Label {
    let label = gtk4::Label::new(Some(text));
    label.set_xalign(0.0);
    label.set_valign(gtk4::Align::Center);
    label
}

fn window_button(class: &str, glyph: &str, tooltip: &str) -> gtk4::Button {
    let button = gtk4::Button::with_label(glyph);
    button.add_css_class("window-button");
    button.add_css_class(class);
    button.set_tooltip_text(Some(tooltip));
    button.set_valign(gtk4::Align::Center);
    button
}

fn keyed_grid() -> gtk4::Grid {
    let grid = gtk4::Grid::new();
    grid.set_column_spacing(12);
    grid.set_row_spacing(10);
    grid.set_hexpand(true);
    grid
}

fn card(title: &str) -> (gtk4::Box, gtk4::Box) {
    let outer = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    outer.add_css_class("card");
    let heading = gtk4::Label::new(Some(title));
    heading.set_xalign(0.0);
    heading.add_css_class("card-title");
    outer.append(&heading);
    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    outer.append(&content);
    (outer, content)
}

fn attach_slider(
    grid: &gtk4::Grid,
    row: i32,
    title: &str,
    min: f64,
    max: f64,
    step: f64,
) -> (gtk4::Scale, gtk4::Label) {
    grid.attach(&field_label(title), 0, row, 1, 1);
    let scale = gtk4::Scale::with_range(gtk4::Orientation::Horizontal, min, max, step);
    scale.set_draw_value(false);
    scale.set_hexpand(true);
    scale.set_valign(gtk4::Align::Center);
    let value = gtk4::Label::new(None);
    value.set_width_chars(5);
    value.set_xalign(1.0);
    value.add_css_class("hint");
    let row_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
    row_box.append(&scale);
    row_box.append(&value);
    grid.attach(&row_box, 1, row, 1, 1);
    (scale, value)
}

fn preview_stage(stage_height: i32, picture_size: i32) -> (gtk4::Overlay, gtk4::Picture) {
    let overlay = gtk4::Overlay::new();
    overlay.add_css_class("preview-stage");
    overlay.set_halign(gtk4::Align::Fill);
    overlay.set_height_request(stage_height);

    let checker = gtk4::DrawingArea::new();
    checker.set_draw_func(|_, cr, width, height| {
        let width = width as usize;
        let height = height as usize;
        let cell = 14usize;
        for y in (0..height).step_by(cell) {
            for x in (0..width).step_by(cell) {
                let even = ((x / cell) + (y / cell)) % 2 == 0;
                let level = if even { 0.08 } else { 0.11 };
                let x_i32 = x as i32;
                let y_i32 = y as i32;
                let cell_i32 = cell as i32;
                cr.set_source_rgb(level, level, level);
                cr.rectangle(
                    f64::from(x_i32),
                    f64::from(y_i32),
                    f64::from(cell_i32),
                    f64::from(cell_i32),
                );
                let _ = cr.fill();
            }
        }
    });
    overlay.set_child(Some(&checker));

    let picture = gtk4::Picture::new();
    picture.set_can_shrink(true);
    picture.set_content_fit(gtk4::ContentFit::Contain);
    picture.set_size_request(picture_size, picture_size);
    picture.set_halign(gtk4::Align::Center);
    picture.set_valign(gtk4::Align::Center);
    overlay.add_overlay(&picture);
    (overlay, picture)
}

pub fn build(
    app: &gtk4::Application,
    controller: &Controller,
) -> (gtk4::ApplicationWindow, Rc<Ui>) {
    let sync = Rc::new(Cell::new(false));
    let window = gtk4::ApplicationWindow::builder()
        .application(app)
        .title(APP_NAME)
        .icon_name(crate::model::APP_ID)
        .default_width(1040)
        .default_height(960)
        .build();
    window.set_hide_on_close(true);
    window.set_size_request(780, 560);

    // Escape leaves full screen: GTK hides the title bar while full screen, so
    // the traffic-light button is not reachable until the window is restored.
    {
        let key_controller = gtk4::EventControllerKey::new();
        let window_for_key = window.clone();
        key_controller.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Escape && window_for_key.is_fullscreen() {
                window_for_key.unfullscreen();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        window.add_controller(key_controller);
    }

    // ------------------------------------------------------------------
    // Header
    // ------------------------------------------------------------------
    let header = gtk4::HeaderBar::new();
    header.set_show_title_buttons(false);
    let window_buttons = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    window_buttons.set_valign(gtk4::Align::Center);
    window_buttons.add_css_class("window-buttons");
    let close_button = window_button("close", "×", "Close");
    let minimize_button = window_button("minimize", "−", "Minimize");
    let fullscreen_button = window_button("maximize", "⤢", "Full screen (Esc to exit)");
    window_buttons.append(&close_button);
    window_buttons.append(&minimize_button);
    window_buttons.append(&fullscreen_button);
    header.pack_start(&window_buttons);
    {
        let window = window.clone();
        close_button.connect_clicked(move |_| window.close());
    }
    {
        let window = window.clone();
        minimize_button.connect_clicked(move |_| window.minimize());
    }
    {
        let window = window.clone();
        fullscreen_button.connect_clicked(move |_| {
            if window.is_fullscreen() {
                window.unfullscreen();
            } else {
                window.fullscreen();
            }
        });
    }

    let brand_icon = gtk4::Image::from_paintable(Some(&gallery::texture_from_pixmap(
        &crate::render::brand_icon(30),
    )));
    let brand_text = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    let title = gtk4::Label::new(Some(APP_NAME));
    title.set_xalign(0.0);
    title.add_css_class("brand-title");
    let subtitle = gtk4::Label::new(Some("Free crosshair overlay for Linux"));
    subtitle.set_xalign(0.0);
    subtitle.add_css_class("brand-sub");
    brand_text.append(&title);
    brand_text.append(&subtitle);
    let brand = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
    brand.append(&brand_icon);
    brand.append(&brand_text);
    header.set_title_widget(Some(&brand));

    let status_pill = gtk4::Label::new(Some("Off"));
    status_pill.add_css_class("status-pill");
    let enable_switch = gtk4::Switch::new();
    enable_switch.set_valign(gtk4::Align::Center);
    enable_switch.set_tooltip_text(Some("Show or hide the crosshair overlay"));

    let menu_button = gtk4::MenuButton::new();
    menu_button.set_icon_name("open-menu-symbolic");
    menu_button.set_tooltip_text(Some("Menu"));
    let popover = gtk4::Popover::new();
    let menu_box = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    menu_box.set_margin_top(6);
    menu_box.set_margin_bottom(6);
    menu_box.set_margin_start(6);
    menu_box.set_margin_end(6);
    let about_button = gtk4::Button::with_label("About Crosshair Linux");
    about_button.set_has_frame(false);
    about_button.set_halign(gtk4::Align::Start);
    let quit_button = gtk4::Button::with_label("Quit");
    quit_button.set_has_frame(false);
    quit_button.set_halign(gtk4::Align::Start);
    menu_box.append(&about_button);
    menu_box.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));
    menu_box.append(&quit_button);
    popover.set_child(Some(&menu_box));
    menu_button.set_popover(Some(&popover));

    header.pack_end(&status_pill);
    header.pack_end(&enable_switch);
    header.pack_end(&menu_button);
    window.set_titlebar(Some(&header));

    // ------------------------------------------------------------------
    // Content scaffolding
    // ------------------------------------------------------------------
    let scroller = gtk4::ScrolledWindow::new();
    scroller.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
    scroller.set_vexpand(true);
    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 14);
    content.set_margin_top(16);
    content.set_margin_bottom(14);
    content.set_margin_start(18);
    content.set_margin_end(18);
    scroller.set_child(Some(&content));
    window.set_child(Some(&scroller));

    let notice_revealer = gtk4::Revealer::new();
    notice_revealer.set_transition_type(gtk4::RevealerTransitionType::SlideDown);
    notice_revealer.set_reveal_child(false);
    let notice_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
    notice_box.add_css_class("notice");
    let notice_icon = gtk4::Image::from_icon_name("dialog-information-symbolic");
    notice_icon.add_css_class("green");
    let notice_label = gtk4::Label::new(None);
    notice_label.set_xalign(0.0);
    notice_label.set_hexpand(true);
    notice_label.set_wrap(true);
    let notice_close = gtk4::Button::from_icon_name("window-close-symbolic");
    notice_close.set_has_frame(false);
    notice_close.set_valign(gtk4::Align::Center);
    notice_box.append(&notice_icon);
    notice_box.append(&notice_label);
    notice_box.append(&notice_close);
    notice_revealer.set_child(Some(&notice_box));
    content.append(&notice_revealer);
    {
        let revealer = notice_revealer.clone();
        notice_close.connect_clicked(move |_| revealer.set_reveal_child(false));
    }

    let columns = gtk4::Box::new(gtk4::Orientation::Horizontal, 14);
    columns.set_vexpand(true);
    let left = gtk4::Box::new(gtk4::Orientation::Vertical, 14);
    left.set_size_request(300, -1);
    let right = gtk4::Box::new(gtk4::Orientation::Vertical, 14);
    right.set_hexpand(true);
    columns.append(&left);
    columns.append(&right);
    content.append(&columns);

    // ------------------------------------------------------------------
    // Preview
    // ------------------------------------------------------------------
    let (preview_card, preview_content) = card("PREVIEW");
    let (stage, preview_picture) = preview_stage(180, 150);
    preview_content.append(&stage);
    let preview_caption = gtk4::Label::new(None);
    preview_caption.set_xalign(0.0);
    preview_caption.set_wrap(true);
    preview_caption.set_max_width_chars(28);
    preview_caption.add_css_class("preview-caption");
    preview_content.append(&preview_caption);
    left.append(&preview_card);

    // ------------------------------------------------------------------
    // Style
    // ------------------------------------------------------------------
    let (style_card, style_content) = card("STYLE");
    let style_grid = keyed_grid();
    let (size_scale, size_value) = attach_slider(&style_grid, 0, "Size", 16.0, 256.0, 1.0);
    let (hue_scale, hue_value) = attach_slider(&style_grid, 1, "Color hue", 0.0, 360.0, 1.0);
    let (rotation_scale, rotation_value) =
        attach_slider(&style_grid, 2, "Rotation", -180.0, 180.0, 1.0);
    let (opacity_scale, opacity_value) =
        attach_slider(&style_grid, 3, "Opacity", 0.05, 1.0, 0.01);

    style_content.append(&style_grid);
    let reset_style = gtk4::Button::with_label("Reset");
    reset_style.add_css_class("toolbar-button");
    reset_style.set_halign(gtk4::Align::Start);
    reset_style.set_tooltip_text(Some(
        "Restore the default size, hue, rotation, and opacity",
    ));
    style_content.append(&reset_style);
    left.append(&style_card);

    // ------------------------------------------------------------------
    // Crosshairs
    // ------------------------------------------------------------------
    let (crosshair_card, crosshair_content) = card("CROSSHAIR");
    let builtin_tab = gtk4::ToggleButton::with_label("Built-in");
    let custom_tab = gtk4::ToggleButton::with_label("Custom");
    custom_tab.set_group(Some(&builtin_tab));
    let starts_custom = controller.config().style.crosshair.starts_with("file:");
    builtin_tab.set_active(!starts_custom);
    custom_tab.set_active(starts_custom);
    let crosshair_tabs = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    crosshair_tabs.add_css_class("segmented");
    for button in [&builtin_tab, &custom_tab] {
        button.set_hexpand(true);
        button.add_css_class("segment-button");
        crosshair_tabs.append(button);
    }
    crosshair_content.append(&crosshair_tabs);

    let tile_flow = gtk4::FlowBox::new();
    tile_flow.set_selection_mode(gtk4::SelectionMode::None);
    tile_flow.set_homogeneous(true);
    tile_flow.set_column_spacing(6);
    tile_flow.set_row_spacing(6);
    tile_flow.set_min_children_per_line(6);
    tile_flow.set_max_children_per_line(12);
    tile_flow.set_valign(gtk4::Align::Start);
    crosshair_content.append(&tile_flow);

    let import_button = gtk4::Button::with_label("Import…");
    import_button.set_tooltip_text(Some("Copy an SVG or PNG into your crosshair collection"));
    let design_button = gtk4::Button::with_label("Design…");
    design_button.set_tooltip_text(Some("Create a crosshair from simple shapes"));
    let reload_button = gtk4::Button::from_icon_name("view-refresh-symbolic");
    reload_button.set_tooltip_text(Some("Rescan the crosshair folder"));
    let crosshair_actions = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    crosshair_actions.append(&import_button);
    crosshair_actions.append(&design_button);
    crosshair_actions.append(&reload_button);
    crosshair_actions.set_visible(false);
    crosshair_content.append(&crosshair_actions);
    right.append(&crosshair_card);

    // ------------------------------------------------------------------
    // Display
    // ------------------------------------------------------------------
    let (display_card, display_content) = card("DISPLAY");
    let all_displays_switch = gtk4::Switch::new();
    all_displays_switch.set_valign(gtk4::Align::Center);
    let all_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
    let all_label = gtk4::Label::new(Some("Show on all displays"));
    all_label.set_xalign(0.0);
    all_label.set_hexpand(true);
    all_row.append(&all_label);
    all_row.append(&all_displays_switch);
    display_content.append(&all_row);
    let display_box = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    display_content.append(&display_box);
    let display_note = gtk4::Label::new(None);
    display_note.set_xalign(0.0);
    display_note.set_wrap(true);
    display_note.add_css_class("hint");
    display_content.append(&display_note);
    right.append(&display_card);

    // ------------------------------------------------------------------
    // Position
    // ------------------------------------------------------------------
    let (position_card, position_content) = card("POSITION");
    let center_button = gtk4::ToggleButton::with_label("Center");
    let custom_button = gtk4::ToggleButton::with_label("Custom");
    let follow_button = gtk4::ToggleButton::with_label("Follow cursor");
    custom_button.set_group(Some(&center_button));
    follow_button.set_group(Some(&center_button));
    let segmented = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    segmented.add_css_class("segmented");
    segmented.add_css_class("linked");
    for button in [&center_button, &custom_button, &follow_button] {
        button.set_hexpand(true);
        button.add_css_class("segment-button");
        segmented.append(button);
    }
    position_content.append(&segmented);

    let offset_grid = keyed_grid();
    let offset_x_label = field_label("X offset");
    let offset_y_label = field_label("Y offset");
    let offset_x = gtk4::SpinButton::with_range(-10000.0, 10000.0, 1.0);
    let offset_y = gtk4::SpinButton::with_range(-10000.0, 10000.0, 1.0);
    for spin in [&offset_x, &offset_y] {
        spin.set_digits(0);
        spin.set_width_chars(7);
        spin.set_numeric(true);
    }
    offset_grid.attach(&offset_x_label, 0, 0, 1, 1);
    offset_grid.attach(&offset_x, 1, 0, 1, 1);
    offset_grid.attach(&offset_y_label, 0, 1, 1, 1);
    offset_grid.attach(&offset_y, 1, 1, 1, 1);
    position_content.append(&offset_grid);

    let position_hint = gtk4::Label::new(None);
    position_hint.set_xalign(0.0);
    position_hint.set_wrap(true);
    position_hint.add_css_class("hint");
    position_content.append(&position_hint);
    let center_offsets = gtk4::Button::with_label("Reset offsets");
    center_offsets.set_halign(gtk4::Align::Start);
    position_content.append(&center_offsets);
    right.append(&position_card);

    // ------------------------------------------------------------------
    // General
    // ------------------------------------------------------------------
    let (general_card, general_content) = card("GENERAL");
    let general_grid = keyed_grid();

    let theme_dropdown =
        gtk4::DropDown::from_strings(&["Dark", "AMOLED dark", "Light"]);
    theme_dropdown.set_hexpand(true);
    general_grid.attach(&field_label("Theme"), 0, 0, 1, 1);
    general_grid.attach(&theme_dropdown, 1, 0, 1, 1);

    let tray_switch = gtk4::Switch::new();
    tray_switch.set_valign(gtk4::Align::Center);
    let tray_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
    let tray_label = gtk4::Label::new(Some("Tray icon"));
    tray_label.set_xalign(0.0);
    tray_label.set_hexpand(true);
    tray_box.append(&tray_label);
    tray_box.append(&tray_switch);
    general_grid.attach(&tray_box, 0, 1, 2, 1);

    let hotkey_entry = gtk4::Entry::new();
    hotkey_entry.set_placeholder_text(Some("Ctrl+Shift+X"));
    hotkey_entry.set_hexpand(true);
    hotkey_entry.set_width_chars(16);
    hotkey_entry.add_css_class("mono");
    let hotkey_apply = gtk4::Button::with_label("Apply");
    let hotkey_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    hotkey_row.append(&hotkey_entry);
    hotkey_row.append(&hotkey_apply);
    general_grid.attach(&field_label("Toggle hotkey"), 0, 2, 1, 1);
    general_grid.attach(&hotkey_row, 1, 2, 1, 1);
    general_content.append(&general_grid);

    let hotkey_status = gtk4::Label::new(None);
    hotkey_status.set_xalign(0.0);
    hotkey_status.set_wrap(true);
    hotkey_status.add_css_class("hint");
    general_content.append(&hotkey_status);

    let backend_label = gtk4::Label::new(None);
    backend_label.set_xalign(0.0);
    backend_label.set_wrap(true);
    backend_label.add_css_class("hint");
    general_content.append(&backend_label);
    let backend_hint = gtk4::Label::new(None);
    backend_hint.set_xalign(0.0);
    backend_hint.set_wrap(true);
    backend_hint.add_css_class("hint");
    general_content.append(&backend_hint);
    left.append(&general_card);

    // ------------------------------------------------------------------
    // Footer
    // ------------------------------------------------------------------
    let footer = gtk4::Label::new(None);
    footer.set_xalign(0.5);
    footer.set_use_markup(true);
    footer.set_wrap(true);
    footer.add_css_class("footer");
    footer.set_markup(&format!(
        "Crosshair Linux {version} · Free &amp; open source · MIT · <a href=\"https://github.com/DexytF3\">GitHub</a> · Made by Dexytf3 · Inspired by <a href=\"https://github.com/dilates/crosshair\">Dilates Crosshair</a>",
        version = APP_VERSION
    ));
    footer.connect_activate_link(|_, uri| {
        if let Err(err) = gio::AppInfo::launch_default_for_uri(uri, None::<&gio::AppLaunchContext>)
        {
            eprintln!("crosshair-linux: could not open {uri}: {err}");
        }
        glib::Propagation::Stop
    });
    content.append(&footer);

    // ------------------------------------------------------------------
    // Notice area (already built above) — only one place
    // ------------------------------------------------------------------

    // ------------------------------------------------------------------
    // Signal handlers
    // ------------------------------------------------------------------
    {
        let controller = controller.clone();
        let sync = sync.clone();
        enable_switch.connect_active_notify(move |switch| {
            if sync.get() {
                return;
            }
            controller.set_enabled(switch.is_active());
        });
    }
    {
        let controller = controller.clone();
        let sync = sync.clone();
        size_scale.connect_value_changed(move |scale| {
            if sync.get() {
                return;
            }
            let size = scale.value().round().clamp(16.0, 256.0) as u32;
            controller.update_style(move |style| style.size = size);
        });
    }
    {
        let controller = controller.clone();
        let sync = sync.clone();
        hue_scale.connect_value_changed(move |scale| {
            if sync.get() {
                return;
            }
            let hue = scale.value() as f32;
            controller.update_style(move |style| style.hue = hue);
        });
    }
    {
        let controller = controller.clone();
        let sync = sync.clone();
        rotation_scale.connect_value_changed(move |scale| {
            if sync.get() {
                return;
            }
            let rotation = scale.value() as f32;
            controller.update_style(move |style| style.rotation = rotation);
        });
    }
    {
        let controller = controller.clone();
        let sync = sync.clone();
        opacity_scale.connect_value_changed(move |scale| {
            if sync.get() {
                return;
            }
            let opacity = scale.value() as f32;
            controller.update_style(move |style| style.opacity = opacity);
        });
    }
    {
        let controller = controller.clone();
        reset_style.connect_clicked(move |_| controller.reset_style());
    }

    {
        let controller = controller.clone();
        let window = window.clone();
        import_button.connect_clicked(move |_| {
            open_import_dialog(&window, &controller);
        });
    }
    {
        let controller = controller.clone();
        let window = window.clone();
        design_button.connect_clicked(move |_| {
            designer::present(&window, &controller);
        });
    }
    {
        let controller = controller.clone();
        reload_button.connect_clicked(move |_| {
            controller.reload_crosshairs();
            controller.notice("Crosshair collection rescanned.");
        });
    }

    {
        let controller = controller.clone();
        let sync = sync.clone();
        all_displays_switch.connect_active_notify(move |switch| {
            if sync.get() {
                return;
            }
            controller.set_show_all(switch.is_active());
        });
    }

    {
        let controller_center = controller.clone();
        let controller_custom = controller.clone();
        let controller_follow = controller.clone();
        let sync_center = sync.clone();
        let sync_custom = sync.clone();
        let sync_follow = sync.clone();
        center_button.connect_toggled(move |button| {
            if sync_center.get() || !button.is_active() {
                return;
            }
            controller_center.set_position_mode(PositionMode::Center);
        });
        custom_button.connect_toggled(move |button| {
            if sync_custom.get() || !button.is_active() {
                return;
            }
            controller_custom.set_position_mode(PositionMode::Pixel);
        });
        follow_button.connect_toggled(move |button| {
            if sync_follow.get() || !button.is_active() {
                return;
            }
            controller_follow.set_position_mode(PositionMode::Follow);
        });
    }
    {
        let controller_x = controller.clone();
        let controller_y = controller.clone();
        let sync_x = sync.clone();
        let sync_y = sync.clone();
        offset_x.connect_value_changed(move |spin| {
            if sync_x.get() {
                return;
            }
            let value = spin.value().round() as i32;
            controller_x.update_position(move |position| position.x = value);
        });
        offset_y.connect_value_changed(move |spin| {
            if sync_y.get() {
                return;
            }
            let value = spin.value().round() as i32;
            controller_y.update_position(move |position| position.y = value);
        });
    }
    {
        let controller = controller.clone();
        center_offsets.connect_clicked(move |_| {
            controller.update_position(|position| {
                position.x = 0;
                position.y = 0;
            });
        });
    }

    {
        let controller = controller.clone();
        let sync = sync.clone();
        theme_dropdown.connect_selected_notify(move |dropdown| {
            if sync.get() {
                return;
            }
            let theme = Theme::ALL
                .get(dropdown.selected() as usize)
                .copied()
                .unwrap_or_default();
            controller.set_theme(theme);
        });
    }
    {
        let controller = controller.clone();
        let sync = sync.clone();
        tray_switch.connect_active_notify(move |switch| {
            if sync.get() {
                return;
            }
            controller.set_tray_enabled(switch.is_active());
        });
    }
    {
        let controller = controller.clone();
        let hotkey_for_button = hotkey_entry.clone();
        hotkey_apply.connect_clicked(move |_| {
            controller.apply_hotkey_entry(&hotkey_for_button);
        });
    }
    {
        let controller = controller.clone();
        hotkey_entry.connect_activate(move |entry| {
            controller.apply_hotkey_entry(entry);
        });
    }

    {
        let controller = controller.clone();
        let popover = popover.clone();
        about_button.connect_clicked(move |_| {
            popover.popdown();
            controller.show_about();
        });
    }
    {
        let controller = controller.clone();
        let popover = popover.clone();
        quit_button.connect_clicked(move |_| {
            popover.popdown();
            controller.quit();
        });
    }

    let customs = gallery::custom_crosshairs(controller.config().custom_dir.as_deref());
    let ui = Rc::new(Ui {
        window: window.clone(),
        sync,
        thumb_cache: RefCell::new(HashMap::new()),
        status_pill,
        enable_switch,
        preview_picture,
        preview_caption,
        size_scale,
        size_value,
        hue_scale,
        hue_value,
        rotation_scale,
        rotation_value,
        opacity_scale,
        opacity_value,
        reset_style,
        builtin_button: builtin_tab,
        custom_button: custom_tab,
        show_custom: Cell::new(starts_custom),
        crosshair_actions,
        tile_flow,
        tiles: RefCell::new(Vec::new()),
        tiles_theme: Cell::new(None),
        tiles_dirty: Cell::new(true),
        customs: RefCell::new(customs),
        display_box,
        display_buttons: RefCell::new(Vec::new()),
        display_ids: RefCell::new(Vec::new()),
        display_note,
        all_displays_switch,
        mode_buttons: RefCell::new(vec![
            (PositionMode::Center, center_button),
            (PositionMode::Pixel, custom_button),
            (PositionMode::Follow, follow_button),
        ]),
        offset_x,
        offset_y,
        offset_x_label,
        offset_y_label,
        position_hint,
        theme_dropdown,
        tray_switch,
        hotkey_entry,
        hotkey_apply,
        hotkey_status,
        backend_label,
        backend_hint,
        notice_revealer,
        notice_label,
        notice_source: RefCell::new(None),
    });

    {
        let ui_handle = ui.clone();
        let controller = controller.clone();
        ui.builtin_button.connect_toggled(move |button| {
            if ui_handle.sync.get() || !button.is_active() {
                return;
            }
            ui_handle.show_custom.set(false);
            ui_handle.tiles_dirty.set(true);
            refresh(&controller, &ui_handle);
        });
    }
    {
        let ui_handle = ui.clone();
        let controller = controller.clone();
        ui.custom_button.connect_toggled(move |button| {
            if ui_handle.sync.get() || !button.is_active() {
                return;
            }
            ui_handle.show_custom.set(true);
            ui_handle.tiles_dirty.set(true);
            refresh(&controller, &ui_handle);
        });
    }
    (window, ui)
}

fn open_import_dialog(window: &gtk4::ApplicationWindow, controller: &Controller) {
    let dialog = gtk4::FileDialog::new();
    dialog.set_title("Import crosshair");
    dialog.set_modal(true);
    let filter = gtk4::FileFilter::new();
    filter.set_name(Some("Crosshair images (SVG, PNG)"));
    filter.add_pattern("*.svg");
    filter.add_pattern("*.png");
    filter.add_pattern("*.SVG");
    filter.add_pattern("*.PNG");
    let filters = gio::ListStore::new::<gtk4::FileFilter>();
    filters.append(&filter);
    dialog.set_filters(Some(&filters));
    dialog.set_default_filter(Some(&filter));

    let hold = dialog.clone();
    let controller = controller.clone();
    dialog.open_multiple(
        Some(window),
        None::<&gio::Cancellable>,
        move |result| {
            let _keep_dialog_alive = &hold;
            match result {
                Ok(files) => {
                    let mut paths = Vec::new();
                    for index in 0..files.n_items() {
                        let Some(file) = files.item(index).and_downcast::<gio::File>() else {
                            continue;
                        };
                        if let Some(path) = file.path() {
                            paths.push(path);
                        }
                    }
                    if paths.is_empty() {
                        return;
                    }
                    match gallery::import_files(&paths) {
                        Ok(imported) => {
                            if let Some(first) = imported.first() {
                                controller.set_crosshair(&format!("file:{}", first.display()));
                            }
                            let count = imported.len();
                            controller.reload_crosshairs();
                            controller.notice(format!(
                                "Imported {count} crosshair{}.",
                                if count == 1 { "" } else { "s" }
                            ));
                        }
                        Err(err) => controller.notice(err),
                    }
                }
                Err(err) => {
                    if !err.matches(gio::IOErrorEnum::Cancelled) {
                        controller.notice(format!("Import failed: {err}"));
                    }
                }
            }
        },
    );
}

fn tile_button(
    ui: &Ui,
    source: &str,
    label: &str,
    tint: Option<[u8; 4]>,
) -> gtk4::Button {
    let button = gtk4::Button::new();
    button.add_css_class("tile");
    button.set_tooltip_text(Some(label));
    button.set_accessible_role(gtk4::AccessibleRole::Button);
    let texture = gallery::thumbnail(&ui.thumb_cache, source, 36, tint);
    let picture = gtk4::Picture::new();
    picture.set_paintable(Some(&texture));
    picture.set_content_fit(gtk4::ContentFit::Contain);
    picture.set_size_request(36, 36);
    button.set_child(Some(&picture));
    button
}

fn rebuild_tiles(controller: &Controller, ui: &Rc<Ui>, theme: Theme) {
    while let Some(child) = ui.tile_flow.first_child() {
        ui.tile_flow.remove(&child);
    }
    let mut tiles = Vec::new();
    if ui.show_custom.get() {
        let customs = ui.customs.borrow().clone();
        for (source, label) in customs {
            let button = tile_button(ui, &source, &label, None);
            let controller = controller.clone();
            let clicked = source.clone();
            button.connect_clicked(move |_| controller.set_crosshair(&clicked));
            ui.tile_flow.insert(&button, -1);
            tiles.push((source, button));
        }
    } else {
        let tint = gallery::theme_tint(theme);
        for (name, _) in assets::BUILTINS {
            let source = format!("builtin:{name}");
            let label = assets::pretty_name(name);
            let button = tile_button(ui, &source, &label, Some(tint));
            let controller = controller.clone();
            let clicked = source.clone();
            button.connect_clicked(move |_| controller.set_crosshair(&clicked));
            ui.tile_flow.insert(&button, -1);
            tiles.push((source, button));
        }
    }
    *ui.tiles.borrow_mut() = tiles;
    ui.tiles_theme.set(Some(theme));
    ui.tiles_dirty.set(false);
}

fn rebuild_display_buttons(controller: &Controller, ui: &Rc<Ui>, displays: &[DisplayInfo]) {
    let ids: Vec<String> = displays.iter().map(|display| display.id.clone()).collect();
    if *ui.display_ids.borrow() == ids {
        return;
    }
    while let Some(child) = ui.display_box.first_child() {
        ui.display_box.remove(&child);
    }
    let mut buttons = Vec::new();
    for display in displays {
        let button = gtk4::Button::new();
        button.add_css_class("display-card");
        button.set_valign(gtk4::Align::Center);
        let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
        let icon = gtk4::Image::from_icon_name("video-display-symbolic");
        row.append(&icon);
        let text = gtk4::Box::new(gtk4::Orientation::Vertical, 1);
        text.set_hexpand(true);
        let name = gtk4::Label::new(Some(&display.name));
        name.set_xalign(0.0);
        name.add_css_class("display-name");
        let resolution = gtk4::Label::new(Some(&format!(
            "{} × {} · scale {}×",
            display.width,
            display.height,
            display.scale.max(1)
        )));
        resolution.set_xalign(0.0);
        resolution.add_css_class("display-res");
        text.append(&name);
        text.append(&resolution);
        row.append(&text);
        if display.primary {
            let badge = gtk4::Label::new(Some("Primary"));
            badge.add_css_class("badge");
            row.append(&badge);
        }
        button.set_child(Some(&row));
        let controller = controller.clone();
        let id = display.id.clone();
        button.connect_clicked(move |_| controller.set_display(Some(id.clone())));
        ui.display_box.append(&button);
        buttons.push((display.id.clone(), button));
    }
    *ui.display_buttons.borrow_mut() = buttons;
    *ui.display_ids.borrow_mut() = ids;
}

fn update_crosshair_tabs(ui: &Ui) {
    if ui.show_custom.get() {
        ui.builtin_button.remove_css_class("active");
        ui.custom_button.add_css_class("active");
    } else {
        ui.custom_button.remove_css_class("active");
        ui.builtin_button.add_css_class("active");
    }
    ui.crosshair_actions.set_visible(ui.show_custom.get());
}

pub fn refresh(controller: &Controller, ui: &Rc<Ui>) {
    let snapshot = controller.snapshot();
    let config = &snapshot.config;
    ui.sync.set(true);

    // Header
    ui.status_pill.set_text(if config.overlay_on { "On" } else { "Off" });
    if config.overlay_on {
        ui.status_pill.add_css_class("on");
    } else {
        ui.status_pill.remove_css_class("on");
    }
    ui.enable_switch.set_active(config.overlay_on);

    // Preview
    let texture = gallery::preview_texture(&config.style, 160);
    ui.preview_picture.set_paintable(Some(&texture));
    ui.preview_caption.set_text(&format!(
        "{} · {} px · {:.0}% opacity",
        gallery::label_for_source(&config.style.crosshair),
        config.style.clamped_size(),
        config.style.clamped_opacity() * 100.0
    ));

    // Style
    ui.size_scale.set_value(f64::from(config.style.clamped_size()));
    ui.size_value
        .set_text(&format!("{} px", config.style.clamped_size()));
    ui.hue_scale.set_value(f64::from(config.style.hue));
    ui.hue_value
        .set_text(&format!("{:.0}°", config.style.hue));
    ui.rotation_scale.set_value(f64::from(config.style.rotation));
    ui.rotation_value
        .set_text(&format!("{:.0}°", config.style.rotation));
    ui.opacity_scale
        .set_value(f64::from(config.style.clamped_opacity()));
    ui.opacity_value.set_text(&format!(
        "{:.0}%",
        config.style.clamped_opacity() * 100.0
    ));

    // Crosshair
    update_crosshair_tabs(ui);
    if ui.tiles_dirty.get() || ui.tiles_theme.get() != Some(config.theme) {
        rebuild_tiles(controller, ui, config.theme);
    }
    for (source, button) in ui.tiles.borrow().iter() {
        if *source == config.style.crosshair {
            button.add_css_class("selected");
        } else {
            button.remove_css_class("selected");
        }
    }

    // Displays
    rebuild_display_buttons(controller, ui, &snapshot.displays);
    ui.all_displays_switch
        .set_active(config.show_on_all_displays);
    ui.all_displays_switch
        .set_sensitive(config.position.mode != PositionMode::Follow);
    let effective = snapshot.effective_display.as_deref();
    for (id, button) in ui.display_buttons.borrow().iter() {
        let selected = effective == Some(id.as_str());
        if selected {
            button.add_css_class("selected");
        } else {
            button.remove_css_class("selected");
        }
        button.set_sensitive(!config.show_on_all_displays);
    }
    let display_note = if snapshot.displays.is_empty() {
        "No displays reported. The overlay backend may not be available.".to_string()
    } else if snapshot.backend == Some(Backend::Wayland) {
        "Native Wayland output via wlr-layer-shell; one surface per selected display.".to_string()
    } else if snapshot.wayland_session {
        "Running through XWayland. GNOME does not implement wlr-layer-shell.".to_string()
    } else {
        "X11 override-redirect overlay; one click-through window per selected display."
            .to_string()
    };
    ui.display_note.set_text(&display_note);

    // Position
    for (mode, button) in ui.mode_buttons.borrow().iter() {
        let active = *mode == config.position.mode;
        button.set_active(active);
        if active {
            button.add_css_class("active");
        } else {
            button.remove_css_class("active");
        }
        if *mode == PositionMode::Follow {
            button.set_sensitive(snapshot.follow_available);
            button.set_tooltip_text(Some(if snapshot.follow_available {
                "Move the crosshair with the pointer"
            } else {
                "Follow cursor is not available on Wayland"
            }));
        }
    }
    ui.offset_x.set_value(f64::from(config.position.x));
    ui.offset_y.set_value(f64::from(config.position.y));
    let (x_label, y_label) = match config.position.mode {
        PositionMode::Pixel => ("X position", "Y position"),
        _ => ("X offset", "Y offset"),
    };
    ui.offset_x_label.set_text(x_label);
    ui.offset_y_label.set_text(y_label);
    let mut hint = match config.position.mode {
        PositionMode::Center => {
            "Coordinates are offsets from the center of the selected display.".to_string()
        }
        PositionMode::Pixel => {
            "Coordinates are measured from the top-left corner of the selected display."
                .to_string()
        }
        PositionMode::Follow => {
            "The crosshair follows the pointer; offsets are relative to the pointer."
                .to_string()
        }
    };
    if snapshot.wayland_session {
        hint.push_str(" Wayland does not support follow-cursor or snap-to-cursor.");
    }
    ui.position_hint.set_text(&hint);

    // General
    ui.theme_dropdown.set_selected(
        Theme::ALL
            .iter()
            .position(|theme| *theme == config.theme)
            .unwrap_or(0) as u32,
    );
    ui.tray_switch.set_active(config.tray);
    if !ui.hotkey_entry.has_focus() {
        ui.hotkey_entry.set_text(&config.toggle_hotkey);
    }
    ui.hotkey_status.remove_css_class("danger");
    if let Some(error) = snapshot.hotkey_error.as_deref() {
        ui.hotkey_status.set_text(error);
        ui.hotkey_status.add_css_class("danger");
    } else if config.toggle_hotkey.trim().is_empty() {
        ui.hotkey_status
            .set_text("Global hotkeys are disabled. Enter a shortcut to enable them.");
    } else if let Some(backend) = snapshot.hotkey_backend {
        ui.hotkey_status.set_text(&format!(
            "Registered via {}. Leave empty to disable.",
            backend.label()
        ));
    } else {
        ui.hotkey_status
            .set_text("The shortcut is not registered yet. Press Apply to retry.");
    }

    let (backend_text, backend_hint) = match snapshot.backend {
        Some(Backend::X11) => (
            format!("Overlay backend: {}", Backend::X11.label()),
            "ARGB override-redirect windows with an empty XShape input region. Requires borderless windowed games.".to_string(),
        ),
        Some(Backend::Wayland) => (
            format!("Overlay backend: {}", Backend::Wayland.label()),
            "Layer-shell overlay surfaces with an empty input region. Follow-cursor is unavailable.".to_string(),
        ),
        None => (
            "Overlay backend: unavailable".to_string(),
            "No supported display server was detected. The settings window and tray still work."
                .to_string(),
        ),
    };
    ui.backend_label.set_text(&backend_text);
    ui.backend_hint.set_text(&backend_hint);

    ui.sync.set(false);
}

pub fn reload_crosshairs(controller: &Controller, ui: &Rc<Ui>) {
    let extra = controller.config().custom_dir;
    let customs = gallery::custom_crosshairs(extra.as_deref());
    *ui.customs.borrow_mut() = customs;
    ui.tiles_dirty.set(true);
    refresh(controller, ui);
}

pub fn show_notice(ui: &Rc<Ui>, message: &str) {
    if let Some(source) = ui.notice_source.borrow_mut().take() {
        // `SourceId::remove` panics if the source already fired, so ignore the
        // return value instead of letting GLib's failure abort the app.
        unsafe {
            glib::ffi::g_source_remove(source.as_raw());
        }
    }
    ui.notice_label.set_text(message);
    ui.notice_revealer.set_reveal_child(true);
    let weak = Rc::downgrade(ui);
    let source = glib::timeout_add_seconds_local_once(7, move || {
        if let Some(ui) = weak.upgrade() {
            ui.notice_source.borrow_mut().take();
            ui.notice_revealer.set_reveal_child(false);
        }
    });
    *ui.notice_source.borrow_mut() = Some(source);
}
