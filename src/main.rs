use crosshair_linux::action::AppAction;
use crosshair_linux::app::controller::Controller;
use crosshair_linux::model::APP_ID;
use crosshair_linux::ui;
use gtk4::prelude::*;
use gtk4::{glib, Application};
use gtk4::gio::ApplicationFlags;

fn main() -> glib::ExitCode {
    gtk4::init().expect("failed to initialize GTK");
    // Used by the X11 dock/window list; Wayland matches the app ID instead.
    gtk4::Window::set_default_icon_name(APP_ID);

    let (tx, rx) = async_channel::bounded::<AppAction>(256);

    let app = Application::builder()
        .application_id(APP_ID)
        .flags(ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();

    let controller = Controller::new(app.clone(), tx);

    {
        let controller = controller.clone();
        app.connect_startup(move |app| {
            let (_window, ui_rc) = ui::window::build(app, &controller);
            controller.bind_ui(ui_rc);
        });
    }

    // Process actions coming from hotkeys, the tray, or remote invocations.
    let ctrl = controller.clone();
    glib::spawn_future_local(async move {
        while let Ok(action) = rx.recv().await {
            ctrl.handle_action(action);
        }
    });

    {
        let controller = controller.clone();
        app.connect_command_line(move |app, command_line| {
            // The argument list starts with the executable path, so look for
            // the first actual option instead of treating the path as one.
            let first = command_line
                .arguments()
                .iter()
                .filter_map(|arg| arg.to_str())
                .find(|arg| arg.starts_with('-'))
                .unwrap_or("")
                .to_string();
            let remote = command_line.is_remote();

            let exit = match first.as_str() {
                "" | "--settings" => {
                    controller.handle_action(AppAction::OpenSettings);
                    glib::ExitCode::new(0)
                }
                "--toggle" => {
                    controller.handle_action(AppAction::Toggle);
                    glib::ExitCode::new(0)
                }
                "--show" => {
                    controller.handle_action(AppAction::Show);
                    glib::ExitCode::new(0)
                }
                "--hide" => {
                    controller.handle_action(AppAction::Hide);
                    glib::ExitCode::new(0)
                }
                "--quit" => {
                    controller.handle_action(AppAction::Quit);
                    glib::ExitCode::new(0)
                }
                "--help" | "-h" => {
                    println!(
                        "Crosshair Linux {}\n\n\
                         Usage: crosshair-linux [option]\n\n\
                         \x20 --settings   Open the settings window (default)\n\
                         \x20 --toggle     Show or hide the crosshair\n\
                         \x20 --show       Force the crosshair to be visible\n\
                         \x20 --hide       Force the crosshair to be hidden\n\
                         \x20 --quit       Exit the running instance\n\
                         \x20 --version    Print the version\n\
                         \x20 --help       Show this help",
                        crosshair_linux::model::APP_VERSION
                    );
                    if !remote {
                        app.quit();
                    }
                    glib::ExitCode::new(0)
                }
                "--version" | "-V" => {
                    println!("{}", crosshair_linux::model::APP_VERSION);
                    if !remote {
                        app.quit();
                    }
                    glib::ExitCode::new(0)
                }
                other => {
                    eprintln!("crosshair-linux: unknown option {other:?}");
                    if !remote {
                        app.quit();
                    }
                    glib::ExitCode::new(2)
                }
            };
            exit
        });
    }

    app.run()
}
