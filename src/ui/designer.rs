//! Visual crosshair designer.
//!
//! The designer edits a small set of parameters, renders the SVG live, and
//! saves the result (plus a TOML sidecar) into the user's crosshair folder.

use crate::app::Controller;
use crate::model::{DesignerCap, DesignerDesign, Style};
use crate::render::{self, RenderOptions, Source};
use crate::ui::gallery;
use gtk4::prelude::*;
use std::rc::Rc;

fn field_label(text: &str) -> gtk4::Label {
    let label = gtk4::Label::new(Some(text));
    label.set_xalign(0.0);
    label
}

fn spin(min: f64, max: f64, step: f64, digits: u32) -> gtk4::SpinButton {
    let spin = gtk4::SpinButton::with_range(min, max, step);
    spin.set_digits(digits);
    spin.set_numeric(true);
    spin.set_width_chars(5);
    spin
}

struct DesignerFields {
    name: gtk4::Entry,
    arm: gtk4::SpinButton,
    gap: gtk4::SpinButton,
    thickness: gtk4::SpinButton,
    dot: gtk4::SpinButton,
    ring: gtk4::SpinButton,
    ring_thickness: gtk4::SpinButton,
    cap: gtk4::DropDown,
    up: gtk4::CheckButton,
    right: gtk4::CheckButton,
    down: gtk4::CheckButton,
    left: gtk4::CheckButton,
    diagonal: gtk4::CheckButton,
    picture: gtk4::Picture,
}

impl DesignerFields {
    fn design(&self) -> DesignerDesign {
        let cap = match self.cap.selected() {
            1 => DesignerCap::Round,
            2 => DesignerCap::Square,
            _ => DesignerCap::Butt,
        };
        DesignerDesign {
            name: self.name.text().trim().to_string(),
            arm_length: self.arm.value() as f32,
            gap: self.gap.value() as f32,
            thickness: self.thickness.value() as f32,
            dot_radius: self.dot.value() as f32,
            ring_radius: self.ring.value() as f32,
            ring_thickness: self.ring_thickness.value() as f32,
            cap: cap.svg_value().to_string(),
            up: self.up.is_active(),
            right: self.right.is_active(),
            down: self.down.is_active(),
            left: self.left.is_active(),
            diagonal: self.diagonal.is_active(),
        }
    }

    fn refresh(&self) {
        let svg = self.design().to_svg();
        let pixmap = render::render(
            &Source::Svg(svg),
            &RenderOptions::from_style(&Style::default(), 190),
        );
        let texture = gallery::texture_from_pixmap(&pixmap);
        self.picture.set_paintable(Some(&texture));
    }
}

fn preview_stage() -> (gtk4::Overlay, gtk4::Picture) {
    let overlay = gtk4::Overlay::new();
    overlay.add_css_class("preview-stage");
    overlay.set_size_request(230, 230);
    overlay.set_valign(gtk4::Align::Start);

    let checker = gtk4::DrawingArea::new();
    checker.set_draw_func(|_, cr, width, height| {
        let width = width as usize;
        let height = height as usize;
        let cell = 14usize;
        for y in (0..height).step_by(cell) {
            for x in (0..width).step_by(cell) {
                let even = ((x / cell) + (y / cell)) % 2 == 0;
                let level = if even { 0.10 } else { 0.13 };
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
    picture.set_size_request(200, 200);
    picture.set_halign(gtk4::Align::Center);
    picture.set_valign(gtk4::Align::Center);
    overlay.add_overlay(&picture);

    (overlay, picture)
}

pub fn present(parent: &gtk4::ApplicationWindow, controller: &Controller) {
    let window = gtk4::ApplicationWindow::builder()
        .application(controller.app())
        .title("Crosshair designer")
        .transient_for(parent)
        .modal(false)
        .default_width(720)
        .default_height(520)
        .build();
    window.set_resizable(true);

    let header = gtk4::HeaderBar::new();
    let cancel = gtk4::Button::with_label("Cancel");
    let save = gtk4::Button::with_label("Save crosshair");
    save.add_css_class("suggested-action");
    header.pack_start(&cancel);
    header.pack_end(&save);
    window.set_titlebar(Some(&header));

    let defaults = DesignerDesign::default();
    let name = gtk4::Entry::new();
    name.set_placeholder_text(Some("For example: Neon dot"));
    name.set_width_chars(24);
    name.set_max_width_chars(40);

    let arm = spin(0.0, 16.0, 0.5, 1);
    arm.set_value(f64::from(defaults.arm_length));
    let gap = spin(0.0, 15.0, 0.5, 1);
    gap.set_value(f64::from(defaults.gap));
    let thickness = spin(0.25, 8.0, 0.25, 2);
    thickness.set_value(f64::from(defaults.thickness));
    let dot = spin(0.0, 8.0, 0.25, 2);
    dot.set_value(f64::from(defaults.dot_radius));
    let ring = spin(0.0, 15.0, 0.5, 1);
    ring.set_value(f64::from(defaults.ring_radius));
    let ring_thickness = spin(0.25, 8.0, 0.25, 2);
    ring_thickness.set_value(f64::from(defaults.ring_thickness));

    let cap = gtk4::DropDown::from_strings(&["Butt", "Round", "Square"]);
    cap.set_selected(0);

    let up = gtk4::CheckButton::with_label("Up");
    let right = gtk4::CheckButton::with_label("Right");
    let down = gtk4::CheckButton::with_label("Down");
    let left = gtk4::CheckButton::with_label("Left");
    let diagonal = gtk4::CheckButton::with_label("Diagonal");
    for (button, active) in [
        (&up, defaults.up),
        (&right, defaults.right),
        (&down, defaults.down),
        (&left, defaults.left),
        (&diagonal, defaults.diagonal),
    ] {
        button.set_active(active);
    }

    let grid = gtk4::Grid::new();
    grid.set_column_spacing(14);
    grid.set_row_spacing(10);
    let rows: [(&str, &gtk4::Widget); 7] = [
        ("Arm length", arm.upcast_ref()),
        ("Center gap", gap.upcast_ref()),
        ("Line thickness", thickness.upcast_ref()),
        ("Center dot", dot.upcast_ref()),
        ("Ring radius", ring.upcast_ref()),
        ("Ring thickness", ring_thickness.upcast_ref()),
        ("Line cap", cap.upcast_ref()),
    ];
    for (row, (text, widget)) in rows.into_iter().enumerate() {
        grid.attach(&field_label(text), 0, row as i32, 1, 1);
        grid.attach(widget, 1, row as i32, 1, 1);
    }

    let arms_label = gtk4::Label::new(Some("Arms"));
    arms_label.set_xalign(0.0);
    arms_label.add_css_class("card-title");
    let arms = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
    for button in [&up, &right, &down, &left, &diagonal] {
        arms.append(button);
    }

    let form = gtk4::Box::new(gtk4::Orientation::Vertical, 14);
    form.set_hexpand(true);
    form.append(&field_label("Name"));
    form.append(&name);
    form.append(&grid);
    form.append(&arms_label);
    form.append(&arms);

    let (stage, picture) = preview_stage();
    let preview_side = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    preview_side.append(&stage);
    let preview_hint = gtk4::Label::new(Some(
        "White shapes are recolored by the hue, fill and outline effects in the main window.",
    ));
    preview_hint.set_wrap(true);
    preview_hint.set_max_width_chars(30);
    preview_hint.add_css_class("hint");
    preview_side.append(&preview_hint);

    let columns = gtk4::Box::new(gtk4::Orientation::Horizontal, 24);
    columns.set_margin_top(18);
    columns.set_margin_bottom(18);
    columns.set_margin_start(18);
    columns.set_margin_end(18);
    columns.append(&form);
    columns.append(&preview_side);

    let scroller = gtk4::ScrolledWindow::new();
    scroller.set_child(Some(&columns));
    scroller.set_vexpand(true);
    window.set_child(Some(&scroller));

    let fields = Rc::new(DesignerFields {
        name,
        arm,
        gap,
        thickness,
        dot,
        ring,
        ring_thickness,
        cap,
        up,
        right,
        down,
        left,
        diagonal,
        picture,
    });
    fields.refresh();

    for spin in [
        &fields.arm,
        &fields.gap,
        &fields.thickness,
        &fields.dot,
        &fields.ring,
        &fields.ring_thickness,
    ] {
        let fields = fields.clone();
        spin.connect_value_changed(move |_| fields.refresh());
    }
    {
        let fields_for_cb = fields.clone();
        fields.cap.connect_selected_notify(move |_| fields_for_cb.refresh());
    }
    for check in [
        &fields.up,
        &fields.right,
        &fields.down,
        &fields.left,
        &fields.diagonal,
    ] {
        let fields = fields.clone();
        check.connect_toggled(move |_| fields.refresh());
    }

    {
        let window = window.clone();
        cancel.connect_clicked(move |_| window.close());
    }
    {
        let window = window.clone();
        let controller = controller.clone();
        let fields = fields.clone();
        save.connect_clicked(move |_| {
            let design = fields.design();
            let name = design.name.clone();
            match gallery::save_design(&design, &name) {
                Ok(path) => {
                    let source = format!("file:{}", path.display());
                    controller.set_crosshair(&source);
                    controller.reload_crosshairs();
                    controller.notice(format!(
                        "Saved \"{name}\" to your crosshair collection."
                    ));
                    window.close();
                }
                Err(err) => controller.notice(err),
            }
        });
    }

    window.present();
}
