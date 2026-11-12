use crate::model::Theme;

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub bg: &'static str,
    pub bg2: &'static str,
    pub card: &'static str,
    pub card_alt: &'static str,
    pub border: &'static str,
    pub border_strong: &'static str,
    pub text: &'static str,
    pub muted: &'static str,
    pub accent: &'static str,
    pub on_accent: &'static str,
    pub accent_soft: &'static str,
    pub danger: &'static str,
    pub green: &'static str,
    pub checker_a: &'static str,
    pub checker_b: &'static str,
}

pub fn palette(theme: Theme) -> Palette {
    match theme {
        Theme::Dark => Palette {
            bg: "#1d1c2a",
            bg2: "#232132",
            card: "#262435",
            card_alt: "#2e2b40",
            border: "#3a3752",
            border_strong: "#4d496b",
            text: "#eeecf7",
            muted: "#a9a5c2",
            accent: "#b7aee4",
            on_accent: "#1d1c2a",
            accent_soft: "rgba(183, 174, 228, 0.16)",
            danger: "#f28b8b",
            green: "#b7aee4",
            checker_a: "#262435",
            checker_b: "#2c2a3c",
        },
        Theme::Amoled => Palette {
            bg: "#000000",
            bg2: "#08070e",
            card: "#0c0b14",
            card_alt: "#141220",
            border: "#211e31",
            border_strong: "#322e4b",
            text: "#f4f2fb",
            muted: "#948fae",
            accent: "#c0b7ec",
            on_accent: "#14111f",
            accent_soft: "rgba(192, 183, 236, 0.12)",
            danger: "#f28b8b",
            green: "#c0b7ec",
            checker_a: "#060509",
            checker_b: "#0b0912",
        },
        Theme::Light => Palette {
            bg: "#f5f3fb",
            bg2: "#ece9f6",
            card: "#ffffff",
            card_alt: "#f8f6fe",
            border: "#ded9ee",
            border_strong: "#c6bfdf",
            text: "#1d1a2a",
            muted: "#69647e",
            accent: "#6d5bd0",
            on_accent: "#ffffff",
            accent_soft: "rgba(109, 91, 208, 0.12)",
            danger: "#dc2626",
            green: "#6d5bd0",
            checker_a: "#f0edf8",
            checker_b: "#eae6f4",
        },
    }
}

pub fn apply(provider: &gtk4::CssProvider, theme: Theme) {
    if let Some(settings) = gtk4::Settings::default() {
        // The app ships its own complete palette. Use the stock Adwaita
        // widgets underneath it so shapes, sizes, and icons stay native and
        // consistent regardless of the desktop's GTK theme.
        if settings.gtk_theme_name().as_deref() != Some("Adwaita") {
            settings.set_gtk_theme_name(Some("Adwaita"));
        }
        settings.set_gtk_application_prefer_dark_theme(matches!(
            theme,
            Theme::Dark | Theme::Amoled
        ));
    }
    let css = css(theme);
    provider.load_from_data(&css);
}

pub fn css(theme: Theme) -> String {
    let p = palette(theme);
    format!(
        r#"
/* Map our palette onto the standard GTK color names so built-in widgets
   (switches, scales, checks, menus) follow the selected theme. */
@define-color accent_color {accent};
@define-color accent_bg_color {accent};
@define-color accent_fg_color {on_accent};
@define-color window_bg_color {bg};
@define-color window_fg_color {text};
@define-color view_bg_color {bg2};
@define-color view_fg_color {text};
@define-color headerbar_bg_color {card};
@define-color headerbar_fg_color {text};
@define-color headerbar_border_color {border};
@define-color headerbar_backdrop_color {card};
@define-color card_bg_color {card};
@define-color card_fg_color {text};
@define-color popover_bg_color {card};
@define-color popover_fg_color {text};
@define-color dialog_bg_color {card};
@define-color dialog_fg_color {text};
@define-color success_color {green};
@define-color destructive_color {danger};

* {{
    outline: none;
    box-shadow: none;
    text-shadow: none;
}}

window {{
    background-color: {bg};
    background-image: none;
    color: {text};
    border-radius: 25px;
}}

/* GTK takes most widget text colors from the desktop theme, which may be a
   dark theme even when the user picks our light palette. Own the text color
   explicitly so all three palettes stay readable. */
label,
button,
entry,
entry > text,
spinbutton,
spinbutton > text,
dropdown > button,
menubutton > button,
popover,
popover label,
tooltip,
tooltip label {{
    color: {text};
}}

/* Give buttons a clear surface and border in every palette. Flat and
   title-bar buttons keep the platform look. */
button:not(.titlebutton) {{
    background-color: {card_alt};
    background-image: none;
    border: 1px solid {border_strong};
    border-radius: 25px;
    box-shadow: none;
    min-height: 24px;
    padding: 4px 10px;
}}

button:not(.titlebutton):hover {{
    background-color: {bg2};
    border-color: {accent};
}}

button:not(.titlebutton):active {{
    background-color: {accent_soft};
    border-color: {accent};
}}

button:disabled {{
    color: {muted};
    background-color: {card_alt};
    border-color: {border};
}}

/* Let the desktop GTK theme draw its own close/minimize/maximize buttons. */
button.titlebutton {{
    background-color: transparent;
    border: none;
    box-shadow: none;
    padding: 0;
    min-width: 16px;
    min-height: 16px;
}}

button.titlebutton:hover,
button.titlebutton:active {{
    background-color: transparent;
    border: none;
    box-shadow: none;
}}

/* Mac-style traffic lights, matching the desktop theme's window buttons. */
headerbar .window-button {{
    min-width: 14px;
    min-height: 14px;
    padding: 0;
    border: none;
    border-radius: 999px;
    box-shadow: none;
    background-image: none;
}}

headerbar .window-button label {{
    color: transparent;
    font-size: 10px;
    font-weight: 800;
    margin: 0;
}}

headerbar .window-button:hover label {{
    color: rgba(0, 0, 0, 0.7);
}}

headerbar .window-button.close {{
    background-color: #ff5f57;
}}

headerbar .window-button.minimize {{
    background-color: #febc2e;
}}

headerbar .window-button.maximize {{
    background-color: #28c840;
}}

button.flat,
button.image-button {{
    background-color: transparent;
    background-image: none;
    border-color: transparent;
    border-radius: 25px;
    box-shadow: none;
}}

headerbar {{
    background-color: {card};
    background-image: none;
    border-bottom: 1px solid {border};
    border-radius: 25px 25px 0 0;
    box-shadow: none;
    min-height: 52px;
}}

headerbar windowtitle {{
    padding: 0;
}}

.brand-title {{
    font-size: 15px;
    font-weight: 700;
    color: {text};
}}

.brand-sub {{
    font-size: 11px;
    color: {muted};
}}

.about-title {{
    font-size: 18px;
    font-weight: 700;
    color: {text};
}}

.about-text {{
    font-size: 12px;
    color: {text};
}}

.about-emphasis {{
    font-size: 12px;
    font-weight: 600;
    color: {text};
}}

.license-text {{
    font-family: monospace;
    font-size: 10px;
    color: {muted};
}}

.card {{
    background-color: {card};
    background-image: none;
    border: 1px solid {border};
    border-radius: 25px;
    padding: 16px;
    box-shadow: none;
}}

.card-title {{
    font-size: 11px;
    font-weight: 700;
    color: {muted};
    letter-spacing: 1px;
}}

.hint {{
    font-size: 12px;
    color: {muted};
}}

.mono {{
    font-family: monospace;
}}

.status-pill {{
    font-size: 11px;
    font-weight: 700;
    padding: 3px 10px;
    border-radius: 999px;
    border: 1px solid {border_strong};
    color: {text};
    background-color: {card_alt};
}}

.status-pill.on {{
    color: {on_accent};
    background-color: {accent};
    border-color: {accent};
}}

switch {{
    background-color: {card_alt};
    background-image: none;
    border: 1px solid {border_strong};
    border-radius: 999px;
    box-shadow: none;
}}

switch > slider {{
    background-color: {text};
    background-image: none;
    border: none;
    border-radius: 999px;
    box-shadow: none;
}}

switch:checked {{
    background-color: {accent};
    border-color: {accent};
}}

switch:checked > slider {{
    background-color: {on_accent};
}}

switch:disabled {{
    border-color: {border};
}}

scale {{
    background-color: transparent;
    background-image: none;
    box-shadow: none;
}}

scale > trough {{
    background-color: {border};
    background-image: none;
    border: none;
    border-radius: 999px;
    box-shadow: none;
    min-height: 4px;
    margin: 0;
}}

scale > trough > highlight {{
    background-color: {accent};
    background-image: none;
    border: none;
    border-radius: 999px;
    box-shadow: none;
    min-height: 4px;
    margin: 0;
}}

scale > trough > slider {{
    background-color: {text};
    background-image: none;
    border: 1px solid {border_strong};
    border-radius: 999px;
    box-shadow: none;
    min-width: 14px;
    min-height: 14px;
    margin: 0;
}}

entry,
spinbutton {{
    background-color: {card_alt};
    background-image: none;
    border: 1px solid {border_strong};
    border-radius: 25px;
    box-shadow: none;
}}

progressbar > trough > progress {{
    background-color: {accent};
}}

checkbutton > check:checked,
checkbutton > check:indeterminate,
radiobutton > radio:checked {{
    background-color: {accent};
    color: {on_accent};
}}

button.suggested-action {{
    background-color: {accent};
    color: {on_accent};
}}

button.suggested-action label {{
    color: {on_accent};
}}

entry:focus-within,
spinbutton:focus-within {{
    border-color: {accent};
}}

popover > contents,
popover.menu > contents {{
    background-color: {card};
    background-image: none;
    border: 1px solid {border};
    border-radius: 25px;
    box-shadow: none;
}}

scrollbar {{
    background-color: transparent;
    background-image: none;
}}

scrollbar slider {{
    background-color: {border_strong};
    background-image: none;
    border-radius: 999px;
    min-width: 6px;
    min-height: 6px;
}}

.preview-stage {{
    background-color: {bg2};
    background-image: none;
    border: 1px solid {border};
    border-radius: 25px;
    box-shadow: none;
}}

.tile {{
    min-width: 52px;
    min-height: 52px;
    padding: 6px;
    background-color: {card_alt};
    background-image: none;
    border: 1px solid {border};
    border-radius: 25px;
    box-shadow: none;
}}

.tile:hover {{
    background-color: {bg2};
    border-color: {border_strong};
}}

.tile.selected {{
    background-color: {accent_soft};
    border: 2px solid {accent};
    padding: 5px;
}}

.display-card {{
    padding: 10px 12px;
    border-radius: 25px;
    background-color: {card_alt};
    background-image: none;
    border: 1px solid {border};
    box-shadow: none;
}}

.display-card:hover {{
    border-color: {border_strong};
}}

.display-card.selected {{
    background-color: {accent_soft};
    border: 2px solid {accent};
    padding: 9px 11px;
}}

.display-name {{
    font-weight: 600;
    font-size: 13px;
}}

.display-res {{
    font-size: 11px;
    color: {muted};
}}

.badge {{
    font-size: 10px;
    font-weight: 700;
    padding: 1px 6px;
    border-radius: 999px;
    background-color: {accent_soft};
    color: {accent};
}}

.footer {{
    color: {muted};
    font-size: 12px;
    padding: 2px 4px 10px 4px;
}}

.notice {{
    background-color: {card_alt};
    background-image: none;
    border: 1px solid {border};
    border-radius: 25px;
    box-shadow: none;
    padding: 9px 12px;
}}

.notice button {{
    min-height: 22px;
    min-width: 22px;
    padding: 2px;
}}

.footer link {{
    color: {accent};
}}

.danger {{
    color: {danger};
    background-color: transparent;
    border: 1px solid {border};
}}

.danger label {{
    color: {danger};
}}

.danger:hover {{
    background-color: alpha({danger}, 0.12);
    border-color: {danger};
}}

.green {{
    color: {green};
}}

.preview-caption {{
    font-size: 11px;
    color: {muted};
}}

.segmented {{
    border-radius: 25px;
}}

.segment-button {{
    background-color: transparent;
    background-image: none;
    border-color: transparent;
    border-radius: 25px;
    box-shadow: none;
    color: {text};
    padding: 6px 12px;
}}

.segment-button:hover {{
    background-color: {card_alt};
}}

.segment-button.active {{
    background-color: {accent};
    background-image: none;
    border-color: {accent};
    color: {on_accent};
}}

.segment-button.active label {{
    color: {on_accent};
}}

.segment-button.active:hover {{
    background-color: {accent};
    border-color: {accent};
}}

.toolbar-button {{
    padding: 4px 9px;
    min-height: 24px;
}}

.entry-invalid {{
    border: 2px solid {danger};
    box-shadow: none;
}}

.designer-grid {{
    border-spacing: 8px;
}}

.about-title {{
    font-size: 20px;
    font-weight: 700;
}}
"#,
        bg = p.bg,
        bg2 = p.bg2,
        card = p.card,
        card_alt = p.card_alt,
        border = p.border,
        border_strong = p.border_strong,
        text = p.text,
        muted = p.muted,
        accent = p.accent,
        on_accent = p.on_accent,
        accent_soft = p.accent_soft,
        danger = p.danger,
        green = p.green,
    )
}
