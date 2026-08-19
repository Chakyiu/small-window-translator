mod app;
mod capture;
mod command;
mod config;
mod hotkey;
mod ipc;
mod permissions;
mod popup;
mod settings;
mod stt;
mod theme;
mod translate;
mod tts;
mod ui;

pub use command::AppCommand;

fn main() {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("settings") => {
            if let Err(err) = ipc::trigger_settings() {
                eprintln!("swtrans: could not reach the running app ({err})");
                eprintln!("Start `swtrans` first.");
                std::process::exit(1);
            }
        }
        Some("translate-selection") | Some("selection_translate") => {
            if let Err(err) = ipc::trigger_translate() {
                eprintln!("swtrans: could not reach the running app ({err})");
                eprintln!("Start `swtrans` first, or check ipc_port in config.toml.");
                std::process::exit(1);
            }
        }
        Some("--help" | "-h" | "help") => print_help(),
        Some(other) => {
            eprintln!("Unknown command: {other}");
            print_help();
            std::process::exit(2);
        }
        None => app::run(),
    }
}

fn print_help() {
    eprintln!(
        "\
swtrans — Small Window Translator

Usage:
  swtrans                      Start the app (opens Settings, stays in tray)
  swtrans settings             Open Settings on a running instance
  swtrans translate-selection  Trigger select-translate (Wayland / scripts)
  swtrans --help               Show this help

Default hotkey: Alt+D (configurable in Settings)
Config: platform project dir / swtrans / config.toml
Wayland: bind a compositor shortcut to `swtrans translate-selection`
"
    );
}
