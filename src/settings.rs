use crate::config::Config;
use crate::permissions;
use crate::theme;
use crate::translate;
use crate::ui;
use crate::update;
use crate::AppCommand;
use gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, KeyBinding, KeyDownEvent, SharedString,
    Window, actions, div, prelude::*, px,
};
use std::sync::mpsc::Sender;

actions!(settings, [CloseSettings, SaveSettings]);

pub enum SettingsEvent {
    Dismiss,
}

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("escape", CloseSettings, Some("Settings")),
        KeyBinding::new("cmd-s", SaveSettings, Some("Settings")),
        KeyBinding::new("ctrl-s", SaveSettings, Some("Settings")),
    ]);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    General,
    Providers,
    Advanced,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    Hotkey,
    DeeplKey,
    OpenAiKey,
    OpenAiBase,
    OpenAiModel,
    WhisperModel,
    LibreEndpoint,
    LibreKey,
    IpcPort,
}

pub struct SettingsView {
    focus: FocusHandle,
    field_focus: FocusHandle,
    page: Page,
    active: Field,
    recording: bool,
    dirty: bool,
    config: Config,
    status: SharedString,
    permission: SharedString,
    permission_ok: bool,
    tx: Sender<AppCommand>,
    embedded: bool,
    update_checking: bool,
    update_message: SharedString,
    update_url: Option<String>,
    update_open_label: Option<&'static str>,
}

impl EventEmitter<SettingsEvent> for SettingsView {}

impl SettingsView {
    pub fn new(cx: &mut Context<Self>, config: Config, tx: Sender<AppCommand>) -> Self {
        Self::create(cx, config, tx, false)
    }

    pub fn embedded(cx: &mut Context<Self>, config: Config, tx: Sender<AppCommand>) -> Self {
        Self::create(cx, config, tx, true)
    }

    fn create(
        cx: &mut Context<Self>,
        config: Config,
        tx: Sender<AppCommand>,
        embedded: bool,
    ) -> Self {
        let perm = permissions::current_status();
        Self {
            focus: cx.focus_handle(),
            field_focus: cx.focus_handle(),
            page: Page::General,
            active: Field::Hotkey,
            recording: false,
            dirty: false,
            permission: SharedString::from(perm.message),
            permission_ok: perm.accessibility_ok,
            config,
            status: SharedString::from("Ready"),
            tx,
            embedded,
            update_checking: false,
            update_message: SharedString::from(format!(
                "Installed version {}",
                update::current_version()
            )),
            update_url: None,
            update_open_label: None,
        }
    }

    fn close(&mut self, _: &CloseSettings, window: &mut Window, cx: &mut Context<Self>) {
        if self.embedded {
            let _ = self.tx.send(AppCommand::CloseEmbeddedSettings);
            cx.emit(SettingsEvent::Dismiss);
            return;
        }
        window.remove_window();
    }

    fn close_click(&mut self, _: &gpui::MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.close(&CloseSettings, window, cx);
    }

    fn save_action(&mut self, _: &SaveSettings, _window: &mut Window, cx: &mut Context<Self>) {
        self.persist(cx);
    }

    fn save(&mut self, _: &gpui::MouseUpEvent, _window: &mut Window, cx: &mut Context<Self>) {
        self.persist(cx);
    }

    fn persist(&mut self, cx: &mut Context<Self>) {
        if self.config.ipc_port == 0 {
            self.status = "IPC port must be between 1 and 65535".into();
            cx.notify();
            return;
        }
        match self.config.save() {
            Ok(path) => {
                self.dirty = false;
                self.status = format!("Saved {}", path.display()).into();
                let _ = self.tx.send(AppCommand::ReloadConfig);
            }
            Err(err) => {
                self.status = format!("Save failed: {err}").into();
            }
        }
        cx.notify();
    }

    fn open_config(&mut self, _: &gpui::MouseUpEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if let Ok(path) = Config::config_path() {
            let _ = self.config.save();
            self.dirty = false;
            cx.open_with_system(&path);
            self.status = format!("Opened {}", path.display()).into();
        }
        cx.notify();
    }

    fn open_accessibility(
        &mut self,
        _: &gpui::MouseUpEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(url) = permissions::accessibility_settings_url() {
            cx.open_url(url);
        }
        let perm = permissions::current_status();
        self.permission = SharedString::from(perm.message);
        self.permission_ok = perm.accessibility_ok;
        cx.notify();
    }

    fn check_update(&mut self, _: &gpui::MouseUpEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if self.update_checking {
            return;
        }
        self.update_checking = true;
        self.update_message = "Checking for updates…".into();
        self.update_url = None;
        self.update_open_label = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async { update::check() })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.update_checking = false;
                match result {
                    Ok(outcome) => {
                        this.update_url = outcome.open_url().map(str::to_string);
                        this.update_open_label = outcome.open_label();
                        this.update_message = outcome.message().into();
                    }
                    Err(err) => {
                        this.update_url = None;
                        this.update_open_label = None;
                        this.update_message = format!("Update check failed: {err}").into();
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn open_update_page(
        &mut self,
        _: &gpui::MouseUpEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(url) = &self.update_url {
            cx.open_url(url);
        }
    }

    fn go_page(&mut self, page: Page, cx: &mut Context<Self>) {
        self.page = page;
        self.recording = false;
        cx.notify();
    }

    fn mark_dirty(&mut self) {
        self.dirty = true;
        self.status = "Unsaved changes".into();
    }

    fn toggle_deepl(&mut self, _: &gpui::MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.config.deepl.enabled = !self.config.deepl.enabled;
        self.mark_dirty();
        cx.notify();
    }

    fn toggle_deepl_pro(&mut self, _: &gpui::MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.config.deepl.use_pro = !self.config.deepl.use_pro;
        self.mark_dirty();
        cx.notify();
    }

    fn toggle_openai(&mut self, _: &gpui::MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.config.openai.enabled = !self.config.openai.enabled;
        self.mark_dirty();
        cx.notify();
    }

    fn toggle_libre(&mut self, _: &gpui::MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.config.libre.enabled = !self.config.libre.enabled;
        self.mark_dirty();
        cx.notify();
    }

    fn toggle_google(&mut self, _: &gpui::MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.config.google.enabled = !self.config.google.enabled;
        self.mark_dirty();
        cx.notify();
    }

    fn start_record(&mut self, _: &gpui::MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.recording = true;
        self.status = "Recording hotkey — press the shortcut".into();
        window.focus(&self.focus);
        cx.notify();
    }

    fn set_language(&mut self, code: &str, cx: &mut Context<Self>) {
        self.config.target_lang = code.to_string();
        self.mark_dirty();
        cx.notify();
    }

    fn focus_field(
        &mut self,
        field: Field,
        _event: &gpui::MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.active = field;
        self.recording = false;
        window.focus(&self.field_focus);
        cx.notify();
    }

    fn on_key(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = ev.keystroke.key.as_str();
        if key == "escape" {
            if self.recording {
                self.recording = false;
                self.status = "Cancelled".into();
                cx.notify();
                return;
            }
            self.close(&CloseSettings, window, cx);
            return;
        }
        if self.recording {
            if let Some(spec) = keystroke_to_hotkey(&ev.keystroke) {
                self.config.hotkey = spec;
                self.recording = false;
                self.mark_dirty();
                self.status = format!("Hotkey set to {}", self.config.hotkey).into();
                cx.notify();
            }
            return;
        }

        if key == "tab" {
            self.active = next_field(self.active, self.page);
            cx.notify();
            return;
        }
        if key == "backspace" || key == "delete" {
            if self.active == Field::IpcPort {
                let mut s = self.config.ipc_port.to_string();
                s.pop();
                self.config.ipc_port = s.parse().unwrap_or(0);
            } else {
                self.active_string().pop();
            }
            self.mark_dirty();
            cx.notify();
            return;
        }
        if let Some(ch) = ev.keystroke.key_char.as_ref() {
            if ch.chars().all(|c| !c.is_control()) {
                if self.active == Field::IpcPort {
                    if ch.chars().all(|c| c.is_ascii_digit()) {
                        let mut s = if self.config.ipc_port == 0 {
                            String::new()
                        } else {
                            self.config.ipc_port.to_string()
                        };
                        s.push_str(ch);
                        if let Ok(port) = s.parse::<u16>() {
                            self.config.ipc_port = port;
                            self.mark_dirty();
                        }
                    }
                } else {
                    self.active_string().push_str(ch);
                    self.mark_dirty();
                }
                cx.notify();
            }
        }
    }

    fn active_string(&mut self) -> &mut String {
        match self.active {
            Field::Hotkey => &mut self.config.hotkey,
            Field::DeeplKey => &mut self.config.deepl.api_key,
            Field::OpenAiKey => &mut self.config.openai.api_key,
            Field::OpenAiBase => &mut self.config.openai.base_url,
            Field::OpenAiModel => &mut self.config.openai.model,
            Field::WhisperModel => &mut self.config.openai.whisper_model,
            Field::LibreEndpoint => &mut self.config.libre.endpoint,
            Field::LibreKey => &mut self.config.libre.api_key,
            Field::IpcPort => unreachable!("port uses numeric editor"),
        }
    }
}

impl Focusable for SettingsView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        ui::page()
            .id("settings")
            .key_context("Settings")
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::close))
            .on_action(cx.listener(Self::save_action))
            .on_key_down(cx.listener(Self::on_key))
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h(px(0.))
                    .child(self.sidebar(cx))
                    .child(self.body(cx)),
            )
            .child(self.footer(cx))
    }
}

impl SettingsView {
    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(theme::SETTINGS_SIDEBAR))
            .h_full()
            .px_3()
            .py_4()
            .gap_1()
            .flex()
            .flex_col()
            .border_r_1()
            .border_color(theme::border())
            .bg(theme::bg())
            .child(
                div()
                    .px_3()
                    .pb_3()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_start()
                    .child(
                        div()
                            .child(ui::heading("swtrans"))
                            .child(ui::subtitle("Settings"))
                            .child(ui::subtitle(format!("v{}", update::current_version()))),
                    )
                    .child(ui::icon_btn(
                        "settings-close",
                        "✕",
                        false,
                        cx.listener(Self::close_click),
                    )),
            )
            .when(self.embedded, |el| {
                el.child(ui::ghost_button(
                    "back-query",
                    "← Query",
                    cx.listener(|this, _, window, cx| {
                        this.close(&CloseSettings, window, cx);
                    }),
                ))
            })
            .child(ui::nav_item(
                "nav-general",
                "General",
                self.page == Page::General,
                cx.listener(|this, _, _, cx| this.go_page(Page::General, cx)),
            ))
            .child(ui::nav_item(
                "nav-providers",
                "Providers",
                self.page == Page::Providers,
                cx.listener(|this, _, _, cx| this.go_page(Page::Providers, cx)),
            ))
            .child(ui::nav_item(
                "nav-advanced",
                "Advanced",
                self.page == Page::Advanced,
                cx.listener(|this, _, _, cx| this.go_page(Page::Advanced, cx)),
            ))
    }

    fn body(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match self.page {
            Page::General => self.general_page(cx),
            Page::Providers => self.providers_page(cx),
            Page::Advanced => self.advanced_page(cx),
        };
        div()
            .id("settings-body")
            .flex()
            .flex_col()
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .p_5()
            .gap_4()
            .overflow_y_scroll()
            .child(content)
    }

    fn general_page(&mut self, cx: &mut Context<Self>) -> gpui::Div {
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(ui::heading("General"))
            .child(ui::subtitle(
                "Select text in any app, then press the hotkey.",
            ))
            .child(ui::banner(
                if self.permission_ok {
                    ui::BannerKind::Ok
                } else {
                    ui::BannerKind::Warn
                },
                self.permission.clone().to_string(),
            ))
            .when(permissions::accessibility_settings_url().is_some(), |el| {
                el.child(ui::ghost_button(
                    "ax",
                    "Open Accessibility settings",
                    cx.listener(Self::open_accessibility),
                ))
            })
            .child(
                ui::card()
                    .child(ui::label("HOTKEY"))
                    .child(ui::field(
                        "hotkey-field",
                        "Select-translate",
                        self.config.hotkey.clone(),
                        "Alt+D",
                        self.active == Field::Hotkey,
                        cx.listener(|this, ev, window, cx| {
                            this.focus_field(Field::Hotkey, ev, window, cx)
                        }),
                    ))
                    .child(ui::ghost_button(
                        "record",
                        if self.recording {
                            "Recording — press keys"
                        } else {
                            "Record hotkey"
                        },
                        cx.listener(Self::start_record),
                    )),
            )
            .child(
                ui::card()
                    .child(ui::label("TARGET LANGUAGE"))
                    .child(ui::subtitle(translate::language_label(
                        &self.config.target_lang,
                    )))
                    .child(language_chips(&self.config.target_lang, cx)),
            )
            .child(
                ui::card()
                    .child(ui::label("UPDATES"))
                    .child(ui::subtitle(self.update_message.clone().to_string()))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .gap_2()
                            .child(ui::ghost_button(
                                "check-update",
                                if self.update_checking {
                                    "Checking…"
                                } else {
                                    "Check for update"
                                },
                                cx.listener(Self::check_update),
                            ))
                            .when(self.update_url.is_some(), |el| {
                                el.child(ui::primary_button(
                                    "open-update",
                                    self.update_open_label.unwrap_or("Open download page"),
                                    cx.listener(Self::open_update_page),
                                ))
                            }),
                    ),
            )
    }

    fn providers_page(&mut self, cx: &mut Context<Self>) -> gpui::Div {
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(ui::heading("Providers"))
            .child(ui::subtitle(
                "Enable one or more services. Queries run in parallel.",
            ))
            .child(
                ui::card()
                    .child(ui::label("DEEPL"))
                    .child(ui::toggle(
                        "deepl-on",
                        "Enabled",
                        self.config.deepl.enabled,
                        cx.listener(Self::toggle_deepl),
                    ))
                    .child(ui::toggle(
                        "deepl-pro",
                        "Use DeepL Pro endpoint",
                        self.config.deepl.use_pro,
                        cx.listener(Self::toggle_deepl_pro),
                    ))
                    .child(ui::field(
                        "deepl-key",
                        "API key",
                        mask_if_long(&self.config.deepl.api_key, self.active == Field::DeeplKey),
                        "DeepL-Auth-Key",
                        self.active == Field::DeeplKey,
                        cx.listener(|this, ev, window, cx| {
                            this.focus_field(Field::DeeplKey, ev, window, cx)
                        }),
                    )),
            )
            .child(
                ui::card()
                    .child(ui::label("OPENAI-COMPATIBLE"))
                    .child(ui::subtitle(
                        "Chat translations plus speech-to-text via /v1/audio/transcriptions (Whisper).",
                    ))
                    .child(ui::toggle(
                        "openai-on",
                        "Enabled",
                        self.config.openai.enabled,
                        cx.listener(Self::toggle_openai),
                    ))
                    .child(ui::field(
                        "openai-key",
                        "API key",
                        mask_if_long(&self.config.openai.api_key, self.active == Field::OpenAiKey),
                        "sk-…",
                        self.active == Field::OpenAiKey,
                        cx.listener(|this, ev, window, cx| {
                            this.focus_field(Field::OpenAiKey, ev, window, cx)
                        }),
                    ))
                    .child(ui::field(
                        "openai-base",
                        "Base URL",
                        self.config.openai.base_url.clone(),
                        "https://api.openai.com/v1",
                        self.active == Field::OpenAiBase,
                        cx.listener(|this, ev, window, cx| {
                            this.focus_field(Field::OpenAiBase, ev, window, cx)
                        }),
                    ))
                    .child(ui::field(
                        "openai-model",
                        "Chat model",
                        self.config.openai.model.clone(),
                        "gpt-4o-mini",
                        self.active == Field::OpenAiModel,
                        cx.listener(|this, ev, window, cx| {
                            this.focus_field(Field::OpenAiModel, ev, window, cx)
                        }),
                    ))
                    .child(ui::field(
                        "whisper-model",
                        "Speech-to-text model",
                        self.config.openai.whisper_model.clone(),
                        "whisper-1",
                        self.active == Field::WhisperModel,
                        cx.listener(|this, ev, window, cx| {
                            this.focus_field(Field::WhisperModel, ev, window, cx)
                        }),
                    )),
            )
            .child(
                ui::card()
                    .child(ui::label("LIBRETRANSLATE"))
                    .child(ui::toggle(
                        "libre-on",
                        "Enabled",
                        self.config.libre.enabled,
                        cx.listener(Self::toggle_libre),
                    ))
                    .child(ui::field(
                        "libre-url",
                        "Endpoint",
                        self.config.libre.endpoint.clone(),
                        "http://localhost:5000",
                        self.active == Field::LibreEndpoint,
                        cx.listener(|this, ev, window, cx| {
                            this.focus_field(Field::LibreEndpoint, ev, window, cx)
                        }),
                    ))
                    .child(ui::field(
                        "libre-key",
                        "API key (optional)",
                        self.config.libre.api_key.clone(),
                        "optional",
                        self.active == Field::LibreKey,
                        cx.listener(|this, ev, window, cx| {
                            this.focus_field(Field::LibreKey, ev, window, cx)
                        }),
                    )),
            )
            .child(
                ui::card()
                    .child(ui::label("GOOGLE (UNOFFICIAL)"))
                    .child(ui::banner(
                        ui::BannerKind::Warn,
                        "No API key. Can break and may violate Google ToS. Off by default.",
                    ))
                    .child(ui::toggle(
                        "google-on",
                        "Enabled",
                        self.config.google.enabled,
                        cx.listener(Self::toggle_google),
                    )),
            )
    }

    fn advanced_page(&mut self, cx: &mut Context<Self>) -> gpui::Div {
        let path = Config::config_path()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "(unknown)".into());
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(ui::heading("Advanced"))
            .child(
                ui::card()
                    .child(ui::label("LOCAL TRIGGER"))
                    .child(ui::subtitle(
                        "Wayland and scripts can call the running app over localhost.",
                    ))
                    .child(ui::field(
                        "ipc-port",
                        "IPC port",
                        self.config.ipc_port.to_string(),
                        "18765",
                        self.active == Field::IpcPort,
                        cx.listener(|this, ev, window, cx| {
                            this.focus_field(Field::IpcPort, ev, window, cx)
                        }),
                    ))
                    .child(ui::subtitle(format!(
                        "swtrans translate-selection   or   curl 127.0.0.1:{}/selection_translate",
                        self.config.ipc_port
                    ))),
            )
            .child(
                ui::card()
                    .child(ui::label("CONFIG FILE"))
                    .child(ui::subtitle(path))
                    .child(ui::ghost_button(
                        "open-cfg",
                        "Open config file",
                        cx.listener(Self::open_config),
                    )),
            )
    }

    fn footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .justify_between()
            .items_center()
            .px_5()
            .py_3()
            .border_t_1()
            .border_color(theme::border())
            .bg(theme::card())
            .child(ui::subtitle(self.status.clone().to_string()))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .items_center()
                    .child(ui::subtitle(if self.dirty { "Unsaved" } else { "" }))
                    .child(ui::ghost_button(
                        "settings-footer-close",
                        if self.embedded { "Back" } else { "Close" },
                        cx.listener(Self::close_click),
                    ))
                    .child(ui::primary_button(
                        "save",
                        "Save",
                        cx.listener(Self::save),
                    )),
            )
    }
}

fn next_field(current: Field, page: Page) -> Field {
    match page {
        Page::General => Field::Hotkey,
        Page::Advanced => Field::IpcPort,
        Page::Providers => match current {
            Field::DeeplKey => Field::OpenAiKey,
            Field::OpenAiKey => Field::OpenAiBase,
            Field::OpenAiBase => Field::OpenAiModel,
            Field::OpenAiModel => Field::WhisperModel,
            Field::WhisperModel => Field::LibreEndpoint,
            Field::LibreEndpoint => Field::LibreKey,
            Field::LibreKey => Field::DeeplKey,
            _ => Field::DeeplKey,
        },
    }
}

fn language_chips(current: &str, cx: &mut Context<SettingsView>) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .flex_wrap()
        .gap_2()
        .children(
            translate::language_cycle()
                .iter()
                .enumerate()
                .map(|(i, code)| {
                    let selected = current == *code;
                    let code_owned = (*code).to_string();
                    ui::chip(
                        ("lang", i),
                        format!(
                            "{}  {}",
                            translate::language_flag(code),
                            translate::language_short(code)
                        ),
                        selected,
                        cx.listener(move |this, _, _, cx| {
                            this.set_language(&code_owned, cx);
                        }),
                    )
                }),
        )
}

fn mask_if_long(value: &str, editing: bool) -> String {
    if editing || value.len() <= 4 {
        value.to_string()
    } else {
        format!("••••{}", &value[value.len().saturating_sub(4)..])
    }
}

fn keystroke_to_hotkey(ks: &gpui::Keystroke) -> Option<String> {
    let key = ks.key.as_str();
    if matches!(
        key,
        "control" | "alt" | "shift" | "cmd" | "meta" | "super" | "fn"
    ) {
        return None;
    }
    let mut parts = Vec::new();
    if ks.modifiers.control {
        parts.push("Ctrl");
    }
    if ks.modifiers.alt {
        parts.push("Alt");
    }
    if ks.modifiers.shift {
        parts.push("Shift");
    }
    if ks.modifiers.platform {
        parts.push("Cmd");
    }
    if parts.is_empty() {
        return None;
    }
    let name = if key.len() == 1 {
        key.to_ascii_uppercase()
    } else {
        let mut c = key.chars();
        match c.next() {
            Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
            None => key.to_string(),
        }
    };
    parts.push(name.as_str());
    Some(parts.join("+"))
}
