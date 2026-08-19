use crate::capture::{self, Selection};
use crate::command::AppCommand;
use crate::config::Config;
use crate::hotkey;
use crate::ipc;
use crate::permissions;
use crate::popup::{self, PopupView};
use crate::settings::{self, SettingsView};
use crate::theme;
use crate::translate;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use gpui::{
    App, Bounds, Context, Focusable, Point, Size, Timer, TitlebarOptions, Window,
    WindowBackgroundAppearance, WindowBounds, WindowHandle, WindowKind, WindowOptions, div, point,
    prelude::*, px, size,
};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};

pub fn run() {
    let config = Config::load();
    let (tx, rx) = mpsc::channel::<AppCommand>();
    let _ = tx.send(AppCommand::OpenSettings);

    ipc::start_server(config.ipc_port, tx.clone());

    gpui::Application::new().run(move |cx: &mut App| {
        permissions::become_accessory();
        popup::bind_keys(cx);
        settings::bind_keys(cx);
        cx.on_action(|_: &QuitApp, cx| cx.quit());
        cx.bind_keys([gpui::KeyBinding::new("cmd-q", QuitApp, None)]);
        cx.bind_keys([gpui::KeyBinding::new("ctrl-q", QuitApp, None)]);

        let tray = build_tray();
        let (tray_icon, translate_id, settings_id, quit_id) = match tray {
            Some(pack) => (
                Some(pack.icon),
                Some(pack.translate_id),
                Some(pack.settings_id),
                Some(pack.quit_id),
            ),
            None => (None, None, None, None),
        };
        let manager = GlobalHotKeyManager::new().ok();
        let registered = manager
            .as_ref()
            .and_then(|m| register_hotkey(m, &config.hotkey));

        let bounds = Bounds {
            origin: point(px(-80.0), px(-80.0)),
            size: size(px(8.0), px(8.0)),
        };
        let _hub = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: None,
                    focus: false,
                    show: false,
                    kind: WindowKind::Normal,
                    is_movable: false,
                    is_resizable: false,
                    is_minimizable: false,
                    window_background: WindowBackgroundAppearance::Transparent,
                    ..Default::default()
                },
                |_window, cx| {
                    cx.new(|cx| {
                        let mut hub = Hub {
                            tx: tx.clone(),
                            rx,
                            config,
                            popup: None,
                            settings: None,
                            _hotkeys: manager,
                            current_hotkey: registered,
                            _tray: tray_icon,
                            translate_id,
                            settings_id,
                            quit_id,
                        };
                        hub.start_loop(cx);
                        hub
                    })
                },
            )
            .expect("keep-alive window");
    });
}

gpui::actions!(app, [QuitApp]);

struct Hub {
    tx: Sender<AppCommand>,
    rx: Receiver<AppCommand>,
    config: Config,
    popup: Option<WindowHandle<PopupView>>,
    settings: Option<WindowHandle<SettingsView>>,
    _hotkeys: Option<GlobalHotKeyManager>,
    current_hotkey: Option<global_hotkey::hotkey::HotKey>,
    _tray: Option<TrayIcon>,
    translate_id: Option<tray_icon::menu::MenuId>,
    settings_id: Option<tray_icon::menu::MenuId>,
    quit_id: Option<tray_icon::menu::MenuId>,
}

impl Hub {
    fn start_loop(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                Timer::after(Duration::from_millis(40)).await;
                let _ = this.update(cx, |this, cx| {
                    this.tick(cx);
                });
            }
        })
        .detach();
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        self.drain_os_events();
        while let Ok(cmd) = self.rx.try_recv() {
            self.handle(cmd, cx);
        }
    }

    fn drain_os_events(&mut self) {
        while let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if event.state == HotKeyState::Pressed {
                if let Some(current) = self.current_hotkey {
                    if event.id == current.id() {
                        let _ = self.tx.send(AppCommand::TranslateSelection);
                    }
                } else {
                    let _ = self.tx.send(AppCommand::TranslateSelection);
                }
            }
        }
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            if self
                .translate_id
                .as_ref()
                .is_some_and(|id| id == event.id())
            {
                let _ = self.tx.send(AppCommand::TranslateSelection);
            } else if self
                .settings_id
                .as_ref()
                .is_some_and(|id| id == event.id())
            {
                let _ = self.tx.send(AppCommand::OpenSettings);
            } else if self.quit_id.as_ref().is_some_and(|id| id == event.id()) {
                let _ = self.tx.send(AppCommand::Quit);
            }
        }
    }

    fn handle(&mut self, cmd: AppCommand, cx: &mut Context<Self>) {
        match cmd {
            AppCommand::TranslateSelection => self.translate_selection(cx),
            AppCommand::OpenSettings => self.open_settings(cx),
            AppCommand::Quit => cx.quit(),
            AppCommand::Retranslate {
                text,
                source_lang,
                target_lang,
            } => self.retranslate(text, source_lang, target_lang, cx),
            AppCommand::ReloadConfig => {
                self.config = Config::load();
                if let Some(manager) = self._hotkeys.as_ref() {
                    if let Some(old) = self.current_hotkey.take() {
                        let _ = manager.unregister(old);
                    }
                    self.current_hotkey = register_hotkey(manager, &self.config.hotkey);
                }
            }
        }
    }

    fn translate_selection(&mut self, cx: &mut Context<Self>) {
        let selection = capture::capture_selection();
        let config = self.config.clone();
        let text = selection.text.clone();
        self.show_popup(selection, cx);

        if text.trim().is_empty() {
            return;
        }

        cx.spawn(async move |this, cx| {
            let results = cx
                .background_executor()
                .spawn(async move { translate::translate_all(&config, &text) })
                .await;
            let _ = this.update(cx, |this, cx| {
                if let Some(popup) = this.popup {
                    let _ = popup.update(cx, |view, _window, cx| {
                        view.set_results(results, cx);
                    });
                }
            });
        })
        .detach();
    }

    fn show_popup(&mut self, selection: Selection, cx: &mut Context<Self>) {
        if let Some(handle) = self.popup.take() {
            let _ = handle.update(cx, |_view, window, _cx| {
                window.remove_window();
            });
        }

        let bounds = popup_bounds(cx, &selection);
        let target = self.config.target_lang.clone();
        let source = self.config.source_lang.clone();
        let tx = self.tx.clone();
        let handle = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: None,
                    focus: true,
                    show: true,
                    kind: WindowKind::PopUp,
                    is_movable: true,
                    is_resizable: true,
                    is_minimizable: false,
                    window_background: WindowBackgroundAppearance::Opaque,
                    window_min_size: Some(size(px(320.0), px(280.0))),
                    ..Default::default()
                },
                |window, cx| {
                    let view = cx.new(|cx| PopupView::new(cx, window, selection, &source, &target, tx));
                    window.focus(&view.focus_handle(cx));
                    view
                },
            )
            .ok();
        self.popup = handle;
    }

    fn retranslate(
        &mut self,
        text: String,
        source_lang: String,
        target_lang: String,
        cx: &mut Context<Self>,
    ) {
        self.config.source_lang = source_lang;
        self.config.target_lang = target_lang;
        let _ = self.config.save();
        if let Some(popup) = &self.popup {
            let _ = popup.update(cx, |view, _window, cx| view.mark_busy(cx));
        }
        if text.trim().is_empty() {
            return;
        }
        let config = self.config.clone();
        cx.spawn(async move |this, cx| {
            let results = cx
                .background_executor()
                .spawn(async move { translate::translate_all(&config, &text) })
                .await;
            let _ = this.update(cx, |this, cx| {
                if let Some(popup) = this.popup {
                    let _ = popup.update(cx, |view, _window, cx| {
                        view.set_results(results, cx);
                    });
                }
            });
        })
        .detach();
    }

    fn open_settings(&mut self, cx: &mut Context<Self>) {
        if let Some(handle) = self.popup {
            let _ = handle.update(cx, |view, window, cx| {
                view.show_embedded_settings(window, cx);
            });
            return;
        }
        self.show_settings_window(cx);
    }

    fn show_settings_window(&mut self, cx: &mut Context<Self>) {
        if let Some(handle) = &self.settings {
            if handle
                .update(cx, |_view, window, _cx| {
                    window.activate_window();
                })
                .is_ok()
            {
                return;
            }
        }

        let config = self.config.clone();
        let tx = self.tx.clone();
        let bounds = Bounds::centered(
            None,
            size(px(theme::SETTINGS_WIDTH), px(theme::SETTINGS_HEIGHT)),
            cx,
        );
        let handle = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Small Window Translator".into()),
                        appears_transparent: false,
                        ..Default::default()
                    }),
                    focus: true,
                    show: true,
                    kind: WindowKind::Normal,
                    is_movable: true,
                    is_resizable: true,
                    window_min_size: Some(size(px(640.0), px(480.0))),
                    ..Default::default()
                },
                |window, cx| {
                    let view = cx.new(|cx| SettingsView::new(cx, config, tx));
                    window.focus(&view.focus_handle(cx));
                    view
                },
            )
            .ok();
        self.settings = handle;
    }
}

impl Render for Hub {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size(px(1.0))
    }
}

fn popup_bounds(cx: &App, selection: &Selection) -> Bounds<gpui::Pixels> {
    let size = Size {
        width: px(theme::POPUP_WIDTH),
        height: px(theme::POPUP_HEIGHT),
    };
    let (mut x, mut y) = if let Some(b) = &selection.bounds {
        (b.x + 12.0, b.y + b.height + 12.0)
    } else if let Some((cx_pos, cy_pos)) = capture::cursor_position() {
        (cx_pos + 12.0, cy_pos + 12.0)
    } else {
        (120.0, 120.0)
    };

    let displays = cx.displays();
        let px_f = |p: gpui::Pixels| f64::from(p);
        let screen = displays
            .iter()
            .map(|d| d.bounds())
            .find(|b| {
                x >= px_f(b.origin.x)
                    && y >= px_f(b.origin.y)
                    && x <= px_f(b.origin.x + b.size.width)
                    && y <= px_f(b.origin.y + b.size.height)
            })
            .or_else(|| displays.first().map(|d| d.bounds()));

        if let Some(screen) = screen {
            let max_x = px_f(screen.origin.x + screen.size.width - size.width);
            let max_y = px_f(screen.origin.y + screen.size.height - size.height);
            x = x.clamp(px_f(screen.origin.x), max_x.max(px_f(screen.origin.x)));
            y = y.clamp(px_f(screen.origin.y), max_y.max(px_f(screen.origin.y)));
        }

    Bounds {
        origin: Point {
            x: px(x as f32),
            y: px(y as f32),
        },
        size,
    }
}

fn register_hotkey(manager: &GlobalHotKeyManager, spec: &str) -> Option<global_hotkey::hotkey::HotKey> {
    match hotkey::parse_hotkey(spec) {
        Ok(hk) => match manager.register(hk) {
            Ok(()) => Some(hk),
            Err(err) => {
                eprintln!("swtrans: failed to register hotkey {spec}: {err}");
                None
            }
        },
        Err(err) => {
            eprintln!("swtrans: invalid hotkey {spec}: {err}");
            None
        }
    }
}

struct TrayPack {
    icon: TrayIcon,
    translate_id: tray_icon::menu::MenuId,
    settings_id: tray_icon::menu::MenuId,
    quit_id: tray_icon::menu::MenuId,
}

fn build_tray() -> Option<TrayPack> {
    let translate = MenuItem::new("Translate selection", true, None);
    let settings = MenuItem::new("Settings", true, None);
    let quit = MenuItem::new("Quit", true, None);
    let translate_id = translate.id().clone();
    let settings_id = settings.id().clone();
    let quit_id = quit.id().clone();
    let menu = Menu::new();
    menu.append(&translate).ok()?;
    let _ = menu.append(&settings);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&quit);

    let icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Small Window Translator")
        .with_icon(make_icon())
        .build()
        .ok()?;
    Some(TrayPack {
        icon,
        translate_id,
        settings_id,
        quit_id,
    })
}

fn make_icon() -> Icon {
    Icon::from_rgba(dict_icon_rgba(), 64, 64).expect("tray icon")
}

/// Open bilingual dictionary: blue cover, cream pages, Latin A + 中.
fn dict_icon_rgba() -> Vec<u8> {
    const W: i32 = 64;
    const H: i32 = 64;
    let mut rgba = vec![0u8; (W * H * 4) as usize];
    let put = |buf: &mut [u8], x: i32, y: i32, c: [u8; 4]| {
        if (0..W).contains(&x) && (0..H).contains(&y) {
            let i = ((y * W + x) * 4) as usize;
            buf[i..i + 4].copy_from_slice(&c);
        }
    };
    let fill = |buf: &mut [u8], x0: i32, y0: i32, x1: i32, y1: i32, c: [u8; 4]| {
        for y in y0..=y1 {
            for x in x0..=x1 {
                put(buf, x, y, c);
            }
        }
    };

    const COVER: [u8; 4] = [0x0a, 0x84, 0xff, 255];
    const COVER_DARK: [u8; 4] = [0x00, 0x55, 0xb8, 255];
    const PAGE: [u8; 4] = [0xf5, 0xf5, 0xf7, 255];
    const RULE: [u8; 4] = [0xc7, 0xc7, 0xcc, 255];
    const INK: [u8; 4] = [0x1c, 0x1c, 0x1e, 255];
    const RIBBON: [u8; 4] = [0xff, 0x45, 0x3a, 255];

    // Rounded cover.
    fill(&mut rgba, 8, 10, 55, 52, COVER);
    fill(&mut rgba, 10, 8, 53, 9, COVER);
    fill(&mut rgba, 10, 53, 53, 54, COVER);
    fill(&mut rgba, 8, 10, 9, 12, [0, 0, 0, 0]);
    fill(&mut rgba, 54, 10, 55, 12, [0, 0, 0, 0]);
    put(&mut rgba, 8, 10, COVER);
    put(&mut rgba, 55, 10, COVER);

    // Open pages + gutter.
    fill(&mut rgba, 12, 14, 30, 46, PAGE);
    fill(&mut rgba, 33, 14, 51, 46, PAGE);
    fill(&mut rgba, 31, 12, 32, 50, COVER_DARK);

    // Page rules.
    for y in [22, 28, 34, 40] {
        fill(&mut rgba, 15, y, 28, y, RULE);
        fill(&mut rgba, 35, y, 48, y, RULE);
    }

    // Latin A on the left page.
    fill(&mut rgba, 19, 18, 23, 18, INK);
    fill(&mut rgba, 18, 19, 18, 26, INK);
    fill(&mut rgba, 24, 19, 24, 26, INK);
    fill(&mut rgba, 18, 22, 24, 22, INK);

    // 中 on the right page.
    fill(&mut rgba, 37, 18, 47, 18, INK);
    fill(&mut rgba, 37, 22, 47, 22, INK);
    fill(&mut rgba, 37, 26, 47, 26, INK);
    fill(&mut rgba, 42, 18, 42, 26, INK);

    // Bookmark ribbon.
    fill(&mut rgba, 44, 46, 47, 58, RIBBON);
    put(&mut rgba, 44, 59, RIBBON);
    put(&mut rgba, 47, 59, RIBBON);
    put(&mut rgba, 45, 60, RIBBON);
    put(&mut rgba, 46, 60, RIBBON);

    rgba
}
