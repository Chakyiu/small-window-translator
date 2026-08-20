#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod app;
mod autostart;
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
mod update;
mod vocab;
mod vocab_page;

pub use command::AppCommand;

fn main() {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("settings") => {
            attach_cli_console();
            if let Err(err) = ipc::trigger_settings() {
                eprintln!("swtrans: could not reach the running app ({err})");
                eprintln!("Start `swtrans` first.");
                std::process::exit(1);
            }
        }
        Some("words") | Some("vocab") => {
            attach_cli_console();
            if let Err(err) = ipc::trigger_words() {
                eprintln!("swtrans: could not reach the running app ({err})");
                eprintln!("Start `swtrans` first.");
                std::process::exit(1);
            }
        }
        Some("translate-selection") | Some("selection_translate") => {
            attach_cli_console();
            if let Err(err) = ipc::trigger_translate() {
                eprintln!("swtrans: could not reach the running app ({err})");
                eprintln!("Start `swtrans` first, or check ipc_port in config.toml.");
                std::process::exit(1);
            }
        }
        Some("--hidden" | "--minimized" | "--autostart") => app::run(false),
        Some("--help" | "-h" | "help") => {
            attach_cli_console();
            print_help();
        }
        Some(other) => {
            attach_cli_console();
            eprintln!("Unknown command: {other}");
            print_help();
            std::process::exit(2);
        }
        None => app::run(!crate::config::Config::has_saved_file()),
    }
}

fn attach_cli_console() {
    #[cfg(windows)]
    windows_cli::attach_parent_console();
}

fn print_help() {
    eprintln!(
        "\
swtrans — Small Window Translator

Usage:
  swtrans                      Start the app (opens Settings, stays in tray)
  swtrans settings             Open Settings on a running instance
  swtrans words                Open saved words on a running instance
  swtrans translate-selection  Trigger select-translate (Wayland / scripts)
  swtrans --help               Show this help

Default hotkey: Ctrl+Alt+D (configurable in Settings)
Config: platform project dir / swtrans / config.toml
Wayland: bind a compositor shortcut to `swtrans translate-selection`
"
    );
}

/// Attach to the parent console so CLI subcommands can print when the release
/// binary is built with `windows_subsystem = "windows"`.
#[cfg(windows)]
mod windows_cli {
    pub fn attach_parent_console() {
        const ATTACH_PARENT_PROCESS: u32 = 0xFFFFFFFF;
        const STD_OUTPUT_HANDLE: u32 = 0xFFFFFFF5;
        const STD_ERROR_HANDLE: u32 = 0xFFFFFFF4;
        const GENERIC_READ: u32 = 0x8000_0000;
        const GENERIC_WRITE: u32 = 0x4000_0000;
        const FILE_SHARE_READ: u32 = 1;
        const FILE_SHARE_WRITE: u32 = 2;
        const OPEN_EXISTING: u32 = 3;
        const INVALID_HANDLE_VALUE: isize = -1;

        unsafe extern "system" {
            fn AttachConsole(dw_process_id: u32) -> i32;
            fn SetStdHandle(n_std_handle: u32, h_handle: *mut core::ffi::c_void) -> i32;
            fn CreateFileA(
                lp_file_name: *const core::ffi::c_char,
                dw_desired_access: u32,
                dw_share_mode: u32,
                lp_security_attributes: *mut core::ffi::c_void,
                dw_creation_disposition: u32,
                dw_flags_and_attributes: u32,
                h_template_file: *mut core::ffi::c_void,
            ) -> *mut core::ffi::c_void;
        }

        unsafe {
            if AttachConsole(ATTACH_PARENT_PROCESS) == 0 {
                return;
            }
            let handle = CreateFileA(
                c"CONOUT$".as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                core::ptr::null_mut(),
                OPEN_EXISTING,
                0,
                core::ptr::null_mut(),
            );
            if handle.is_null() || handle as isize == INVALID_HANDLE_VALUE {
                return;
            }
            let _ = SetStdHandle(STD_OUTPUT_HANDLE, handle);
            let _ = SetStdHandle(STD_ERROR_HANDLE, handle);
        }
    }
}
