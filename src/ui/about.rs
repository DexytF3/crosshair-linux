//! Minimal About window.
//!
//! `GtkAboutDialog` renders its comments as plain text, so a small custom
//! window is used here to make the attribution links clickable.

use crate::model::{APP_NAME, APP_VERSION};
use crate::ui::gallery;
use gtk4::prelude::*;
use gtk4::{gdk, gio, glib};

const DILATES_URL: &str = "https://github.com/dilates/crosshair";
const AUTHOR_URL: &str = "https://github.com/DexytF3";
const LICENSE_TEXT: &str = include_str!("../../LICENSE");

pub fn present(parent: &gtk4::ApplicationWindow) {
    let window = gtk4::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title(format!("About {APP_NAME}"))
        .default_width(470)
        .resizable(false)
        .build();

    let header = gtk4::HeaderBar::new();
    header.set_show_title_buttons(false);
    header.set_title_widget(Some(&gtk4::Label::new(Some("About"))));
    let header_close = gtk4::Button::from_icon_name("window-close-symbolic");
    header_close.set_has_frame(false);
    header_close.set_tooltip_text(Some("Close"));
    header_close.add_css_class("flat");
    header.pack_end(&header_close);
    window.set_titlebar(Some(&header));
    {
        let window = window.clone();
        header_close.connect_clicked(move |_| window.close());
    }

    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    content.set_margin_top(22);
    content.set_margin_bottom(22);
    content.set_margin_start(26);
    content.set_margin_end(26);

    let logo = gtk4::Image::from_paintable(Some(&gallery::texture_from_pixmap(
        &crate::render::brand_icon(96),
    )));
    logo.set_size_request(96, 96);
    logo.set_pixel_size(96);
    logo.set_halign(gtk4::Align::Center);
    content.append(&logo);

    let name = gtk4::Label::new(Some(APP_NAME));
    name.add_css_class("about-title");
    content.append(&name);

    let version = gtk4::Label::new(Some(&format!("Version {APP_VERSION}")));
    version.add_css_class("hint");
    content.append(&version);

    content.append(&paragraph("A minimal, native crosshair overlay for Linux."));

    content.append(&link_label(&format!(
        "This project is a fork of <a href=\"{DILATES_URL}\">Dilates Crosshair</a>. \
         The original app is Electron-based, while this one is vibe-coded in Rust with \
         GTK4 for a much smaller memory and storage footprint."
    )));

    let inspired = link_label(&format!(
        "Inspired by <a href=\"{DILATES_URL}\">Dilates Crosshair</a>."
    ));
    inspired.add_css_class("about-emphasis");
    content.append(&inspired);

    content.append(&link_label(&format!(
        "Created by <a href=\"{AUTHOR_URL}\">Dexytf3</a> · \
         <a href=\"{AUTHOR_URL}\">{AUTHOR_URL}</a>"
    )));

    content.append(&license_expander());

    let enjoy = gtk4::Label::new(Some("Enjoy the app. Made with Love."));
    enjoy.add_css_class("about-emphasis");
    ensure_centered(&enjoy);
    content.append(&enjoy);

    let close = gtk4::Button::with_label("Close");
    close.set_halign(gtk4::Align::End);
    close.set_margin_top(6);
    content.append(&close);
    {
        let window = window.clone();
        close.connect_clicked(move |_| window.close());
    }

    window.set_child(Some(&content));

    let keys = gtk4::EventControllerKey::new();
    let window_for_key = window.clone();
    keys.connect_key_pressed(move |_, key, _, _| {
        if key == gdk::Key::Escape {
            window_for_key.close();
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    window.add_controller(keys);

    window.present();
}

fn ensure_centered(label: &gtk4::Label) {
    label.set_wrap(true);
    label.set_xalign(0.5);
    label.set_justify(gtk4::Justification::Center);
}

fn paragraph(text: &str) -> gtk4::Label {
    let label = gtk4::Label::new(Some(text));
    ensure_centered(&label);
    label.add_css_class("about-text");
    label
}

fn link_label(markup: &str) -> gtk4::Label {
    let label = gtk4::Label::new(None);
    label.set_markup(markup);
    ensure_centered(&label);
    label.add_css_class("about-text");
    label.connect_activate_link(|_, uri| {
        if let Err(err) = gio::AppInfo::launch_default_for_uri(uri, None::<&gio::AppLaunchContext>)
        {
            eprintln!("crosshair-linux: could not open {uri}: {err}");
        }
        glib::Propagation::Stop
    });
    label
}

fn license_expander() -> gtk4::Expander {
    let expander = gtk4::Expander::new(Some("MIT License · © Dexytf3"));

    let license = gtk4::Label::new(Some(LICENSE_TEXT));
    license.set_wrap(true);
    license.set_xalign(0.0);
    license.set_selectable(true);
    license.add_css_class("license-text");

    let scroller = gtk4::ScrolledWindow::new();
    scroller.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
    scroller.set_min_content_height(140);
    scroller.set_max_content_height(220);
    scroller.set_child(Some(&license));

    expander.set_child(Some(&scroller));
    expander.set_margin_top(4);
    expander
}
