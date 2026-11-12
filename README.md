# Crosshair Linux

A minimal crosshair overlay for Linux, written in Rust with GTK4.

This project is a fork of [**Dilates Crosshair**](https://github.com/dilates/crosshair).
The original app is Electron-based; this fork is vibe-coded in Rust with GTK4 for
a much smaller memory and storage footprint, and it keeps the settings UI simple:
preview, style, crosshair picker, display, position, and a few general options.

## Features

- Built-in crosshair pack plus support for your own SVG/PNG crosshairs
- Size, color hue, rotation, and opacity controls with a one-click **Reset**
- Center, custom offset, and follow-cursor positioning
- X11/XWayland and wlroots Wayland overlay backends
- System tray and a global show/hide hotkey
- Dark, AMOLED dark, and light palettes

## Build

Dependencies: Rust 1.85+, GTK 4.10+, and the usual X11/Wayland development headers.

```sh
cargo build --release
./target/release/crosshair-linux
```

Start with `--settings` to open the settings window directly.

## Attribution

- Inspired by [**Dilates Crosshair**](https://github.com/dilates/crosshair) (Electron original)
- Fork maintained by **Dexytf3** — <https://github.com/DexytF3>
- Licensed under the MIT License. See [LICENSE](LICENSE).

Enjoy the app. Made with Love.
