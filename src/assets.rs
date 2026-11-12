macro_rules! builtin {
    ($name:literal) => {
        ($name, include_str!(concat!("../data/crosshairs/", $name, ".svg")))
    };
}

/// The built-in crosshair pack. Designs come from Dilates Crosshair (MIT).
pub static BUILTINS: &[(&str, &str)] = &[
    builtin!("arrow-down"),
    builtin!("bracket"),
    builtin!("brackets-dot"),
    builtin!("chevron"),
    builtin!("circle-cross"),
    builtin!("circle-dot"),
    builtin!("circle-plus"),
    builtin!("cross"),
    builtin!("cross-dot"),
    builtin!("cross-thick"),
    builtin!("diamond"),
    builtin!("dot"),
    builtin!("dot-outline"),
    builtin!("dot-ring"),
    builtin!("duplex"),
    builtin!("gap-cross"),
    builtin!("half-cross"),
    builtin!("micro-dot"),
    builtin!("plus"),
    builtin!("quad-dot"),
    builtin!("scope"),
    builtin!("square"),
    builtin!("star"),
    builtin!("t-cross"),
    builtin!("three-dot"),
    builtin!("triangle"),
    builtin!("x"),
    builtin!("x-dot"),
];

pub fn builtin_svg(name: &str) -> Option<&'static str> {
    BUILTINS
        .iter()
        .find_map(|(n, svg)| (*n == name).then_some(*svg))
}

pub fn builtin_source(name: &str) -> String {
    format!("builtin:{name}")
}

pub fn pretty_name(filename: &str) -> String {
    let stem = filename
        .rsplit_once('.')
        .map(|(s, _)| s)
        .unwrap_or(filename);
    stem.replace(['-', '_'], " ")
}

pub static ICON_PNG: &[u8] = include_bytes!("../assets/icon.png");
