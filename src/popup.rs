use crate::capture::Selection;
use crate::config::Config;
use crate::settings::{SettingsEvent, SettingsView};
use crate::stt;
use crate::theme;
use crate::tts;
use crate::translate::{self, TranslateResult};
use crate::ui;
use crate::AppCommand;
use gpui::{
    App, ClipboardItem, Context, Entity, FocusHandle, Focusable, KeyBinding, KeyDownEvent,
    MouseButton, SharedString, Subscription, Timer, Window, actions, div, prelude::*, px, size,
};
use std::sync::mpsc::Sender;
use std::time::Duration;

actions!(popup, [Dismiss, ConfirmQuery]);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("escape", Dismiss, Some("Popup")),
        KeyBinding::new("enter", ConfirmQuery, Some("Popup")),
    ]);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Picker {
    None,
    Source,
    Target,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SpeakTarget {
    Query,
    Result(usize),
}

pub struct PopupView {
    focus: FocusHandle,
    query: String,
    source_lang: String,
    target_lang: String,
    detected: String,
    status: SharedString,
    results: Vec<TranslateResult>,
    collapsed: Vec<String>,
    picker: Picker,
    pinned: bool,
    editing: bool,
    permission_hint: Option<SharedString>,
    can_dismiss: bool,
    tx: Sender<AppCommand>,
    show_settings: bool,
    settings: Option<Entity<SettingsView>>,
    _settings_sub: Option<Subscription>,
    recording: bool,
    mic: Option<stt::Session>,
    speaking: Option<SpeakTarget>,
    _activation: Option<Subscription>,
}

impl PopupView {
    pub fn new(
        cx: &mut Context<Self>,
        window: &mut Window,
        selection: Selection,
        source_lang: &str,
        target_lang: &str,
        tx: Sender<AppCommand>,
    ) -> Self {
        let query = selection.text.trim().to_string();
        let hint = if query.is_empty() {
            let status = crate::permissions::current_status();
            if !status.accessibility_ok {
                Some(SharedString::from(status.message))
            } else {
                Some(SharedString::from(
                    "No text selected. Highlight text, then press the hotkey.",
                ))
            }
        } else {
            None
        };

        let activation = cx.observe_window_activation(window, |this, window, cx| {
            if this.can_dismiss && !this.pinned && !window.is_window_active() {
                this.can_dismiss = false;
                cx.defer_in(window, |_, window, _cx| {
                    window.remove_window();
                });
            }
        });

        cx.spawn(async move |this, cx| {
            Timer::after(Duration::from_millis(280)).await;
            let _ = this.update(cx, |this, _cx| {
                this.can_dismiss = true;
            });
        })
        .detach();

        let detected = if query.is_empty() {
            "auto".to_string()
        } else {
            translate::detect_source(&query, "auto")
        };

        Self {
            focus: cx.focus_handle(),
            query,
            source_lang: source_lang.to_string(),
            target_lang: target_lang.to_string(),
            detected,
            status: if selection.text.trim().is_empty() {
                "Waiting".into()
            } else {
                "Translating…".into()
            },
            results: Vec::new(),
            collapsed: Vec::new(),
            picker: Picker::None,
            pinned: false,
            editing: false,
            permission_hint: hint,
            can_dismiss: false,
            tx,
            show_settings: false,
            settings: None,
            _settings_sub: None,
            recording: false,
            mic: None,
            speaking: None,
            _activation: Some(activation),
        }
    }

    pub fn set_results(&mut self, results: Vec<TranslateResult>, cx: &mut Context<Self>) {
        let ok = results.iter().filter(|r| r.output.is_ok()).count();
        self.status = format!("{ok}/{} providers", results.len()).into();
        self.results = results;
        cx.notify();
    }

    pub fn mark_busy(&mut self, cx: &mut Context<Self>) {
        self.status = "Translating…".into();
        self.results.clear();
        cx.notify();
    }

    fn dismiss(&mut self, _: &Dismiss, window: &mut Window, cx: &mut Context<Self>) {
        tts::stop();
        self.speaking = None;
        if self.recording {
            self.recording = false;
            self.mic = None;
        }
        if self.show_settings {
            self.hide_settings(window, cx);
            return;
        }
        if self.picker != Picker::None {
            self.picker = Picker::None;
            cx.notify();
            return;
        }
        window.remove_window();
    }

    fn confirm(&mut self, _: &ConfirmQuery, _window: &mut Window, cx: &mut Context<Self>) {
        self.editing = false;
        self.request_translate();
        cx.notify();
    }

    fn close_click(&mut self, _: &gpui::MouseUpEvent, window: &mut Window, _cx: &mut Context<Self>) {
        tts::stop();
        window.remove_window();
    }

    fn toggle_pin(&mut self, _: &gpui::MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.pinned = !self.pinned;
        self.status = if self.pinned {
            "Pinned".into()
        } else {
            "Unpinned".into()
        };
        cx.notify();
    }

    fn open_settings(&mut self, _: &gpui::MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.show_embedded_settings(window, cx);
    }

    pub fn show_embedded_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.can_dismiss = false;
        self.pinned = true;
        self.picker = Picker::None;
        let config = Config::load();
        let tx = self.tx.clone();
        let entity = cx.new(|cx| SettingsView::embedded(cx, config, tx));
        self._settings_sub = Some(cx.subscribe_in(
            &entity,
            window,
            |this, _, _: &SettingsEvent, window, cx| {
                this.hide_settings(window, cx);
            },
        ));
        self.settings = Some(entity.clone());
        self.show_settings = true;
        window.resize(size(
            px(theme::SETTINGS_WIDTH),
            px(theme::SETTINGS_HEIGHT),
        ));
        cx.focus_view(&entity, window);
        cx.notify();
    }

    fn hide_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show_settings = false;
        self.settings = None;
        self._settings_sub = None;
        window.resize(size(px(theme::POPUP_WIDTH), px(theme::POPUP_HEIGHT)));
        window.focus(&self.focus);
        cx.notify();
    }

    fn copy_query(&mut self, _: &gpui::MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        if !self.query.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(self.query.clone()));
            self.status = "Copied".into();
            cx.notify();
        }
    }

    fn speak_query(&mut self, _: &gpui::MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        let lang = self.display_source_code().to_string();
        let text = self.query.clone();
        self.toggle_speak(SpeakTarget::Query, &text, &lang, cx);
    }

    fn speak_result(
        &mut self,
        index: usize,
        _: &gpui::MouseUpEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(TranslateResult {
            output: Ok(text), ..
        }) = self.results.get(index)
        else {
            return;
        };
        let text = text.clone();
        let lang = self.target_lang.clone();
        self.toggle_speak(SpeakTarget::Result(index), &text, &lang, cx);
    }

    fn toggle_speak(&mut self, target: SpeakTarget, text: &str, lang: &str, cx: &mut Context<Self>) {
        if self.speaking == Some(target) && tts::is_speaking() {
            tts::stop();
            self.speaking = None;
            self.status = "Stopped".into();
            cx.notify();
            return;
        }
        match tts::speak(text, lang) {
            Ok(()) => {
                self.speaking = Some(target);
                self.status = "Speaking".into();
                cx.spawn(async move |this, cx| {
                    while tts::is_speaking() {
                        Timer::after(Duration::from_millis(150)).await;
                    }
                    let _ = this.update(cx, |this, cx| {
                        if this.speaking == Some(target) {
                            this.speaking = None;
                            cx.notify();
                        }
                    });
                })
                .detach();
            }
            Err(err) => {
                self.speaking = None;
                self.status = format!("TTS: {err}").into();
            }
        }
        cx.notify();
    }

    fn clear_query(&mut self, _: &gpui::MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.query.clear();
        self.results.clear();
        self.permission_hint = None;
        self.editing = true;
        self.status = "Cleared".into();
        cx.notify();
    }

    fn focus_query(&mut self, _: &gpui::MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.editing = true;
        self.picker = Picker::None;
        window.focus(&self.focus);
        cx.notify();
    }

    fn toggle_mic(&mut self, _: &gpui::MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.recording {
            self.stop_and_transcribe(cx);
        } else {
            self.start_recording(cx);
        }
    }

    fn start_recording(&mut self, cx: &mut Context<Self>) {
        let cfg = Config::load();
        if !stt::stt_ready(&cfg.openai) {
            self.status = "Add an OpenAI API key in Settings for speech-to-text".into();
            cx.notify();
            return;
        }
        match stt::Session::start() {
            Ok(session) => {
                self.mic = Some(session);
                self.recording = true;
                self.pinned = true;
                self.can_dismiss = false;
                self.status = "Listening… click mic to stop".into();
                cx.spawn(async move |this, cx| {
                    Timer::after(Duration::from_secs(stt::MAX_SECONDS)).await;
                    let _ = this.update(cx, |this, cx| {
                        if this.recording {
                            this.stop_and_transcribe(cx);
                        }
                    });
                })
                .detach();
            }
            Err(err) => {
                self.status = format!("Mic: {err}").into();
            }
        }
        cx.notify();
    }

    fn stop_and_transcribe(&mut self, cx: &mut Context<Self>) {
        self.recording = false;
        let Some(session) = self.mic.take() else {
            cx.notify();
            return;
        };
        self.status = "Transcribing…".into();
        cx.notify();

        let wav = match session.finish() {
            Ok(bytes) => bytes,
            Err(err) => {
                self.status = format!("{err}").into();
                cx.notify();
                return;
            }
        };

        let cfg = Config::load();
        let lang = self.source_lang.clone();
        cx.spawn(async move |this, cx| {
            let text = cx
                .background_executor()
                .spawn(async move { stt::transcribe(&cfg.openai, &wav, &lang) })
                .await;
            let _ = this.update(cx, |this, cx| match text {
                Ok(spoken) => {
                    this.query = spoken;
                    this.editing = false;
                    this.status = "Transcribed".into();
                    this.request_translate();
                    cx.notify();
                }
                Err(err) => {
                    this.status = format!("STT: {err}").into();
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn copy_result(
        &mut self,
        index: usize,
        _: &gpui::MouseUpEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(TranslateResult {
            output: Ok(text), ..
        }) = self.results.get(index)
        {
            cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
            self.status = format!("Copied {}", self.results[index].provider).into();
            cx.notify();
        }
    }

    fn toggle_card(&mut self, provider: &str, cx: &mut Context<Self>) {
        if let Some(i) = self.collapsed.iter().position(|p| p == provider) {
            self.collapsed.remove(i);
        } else {
            self.collapsed.push(provider.to_string());
        }
        cx.notify();
    }

    fn toggle_picker(&mut self, side: Picker, cx: &mut Context<Self>) {
        self.picker = if self.picker == side {
            Picker::None
        } else {
            side
        };
        cx.notify();
    }

    fn pick_lang(&mut self, side: Picker, code: &str, cx: &mut Context<Self>) {
        match side {
            Picker::Source => self.source_lang = code.to_string(),
            Picker::Target => self.target_lang = code.to_string(),
            Picker::None => {}
        }
        self.picker = Picker::None;
        self.request_translate();
        cx.notify();
    }

    fn swap_langs(&mut self, _: &gpui::MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        let src = if self.source_lang == "auto" {
            self.detected.clone()
        } else {
            self.source_lang.clone()
        };
        if src == "auto" || src.is_empty() {
            self.status = "Detect a language before swapping".into();
            cx.notify();
            return;
        }
        self.source_lang = self.target_lang.clone();
        self.target_lang = src;
        self.picker = Picker::None;
        self.request_translate();
        cx.notify();
    }

    fn request_translate(&mut self) {
        if self.query.trim().is_empty() {
            self.status = "Enter text to translate".into();
            return;
        }
        self.detected = translate::detect_source(&self.query, "auto");
        self.status = "Translating…".into();
        self.results.clear();
        let _ = self.tx.send(AppCommand::Retranslate {
            text: self.query.clone(),
            source_lang: self.source_lang.clone(),
            target_lang: self.target_lang.clone(),
        });
    }

    fn on_key(&mut self, ev: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if !self.editing {
            return;
        }
        let key = ev.keystroke.key.as_str();
        if key == "backspace" || key == "delete" {
            self.query.pop();
            cx.notify();
            return;
        }
        if let Some(ch) = ev.keystroke.key_char.as_ref() {
            if ch.chars().all(|c| !c.is_control()) {
                self.query.push_str(ch);
                cx.notify();
            }
        }
    }

    fn display_source_code(&self) -> &str {
        if self.source_lang == "auto" {
            if self.detected == "auto" {
                "auto"
            } else {
                &self.detected
            }
        } else {
            &self.source_lang
        }
    }
}

impl Focusable for PopupView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for PopupView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.show_settings {
            if let Some(settings) = self.settings.clone() {
                return ui::page()
                    .id("popup")
                    .key_context("Popup")
                    .track_focus(&self.focus)
                    .on_action(cx.listener(Self::dismiss))
                    .child(settings)
                    .into_any_element();
            }
        }

        ui::page()
            .id("popup")
            .key_context("Popup")
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::dismiss))
            .on_action(cx.listener(Self::confirm))
            .on_key_down(cx.listener(Self::on_key))
            .overflow_y_scroll()
            .child(self.chrome(cx))
            .child(self.query_block(cx))
            .child(self.lang_bar(cx))
            .children(self.picker_panel(cx))
            .children(self.permission_hint.clone().map(|hint| {
                div()
                    .px_3()
                    .pt_2()
                    .child(ui::banner(ui::BannerKind::Warn, hint.to_string()))
            }))
            .child(self.results_list(cx))
            .into_any_element()
    }
}

impl PopupView {
    fn chrome(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .justify_between()
            .items_center()
            .px_3()
            .pt_2()
            .pb_1()
            .child(ui::icon_btn(
                "pin",
                if self.pinned { "◉" } else { "⊙" },
                self.pinned,
                cx.listener(Self::toggle_pin),
            ))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_1()
                    .child(ui::icon_btn(
                        "popup-open-settings",
                        "☰",
                        false,
                        cx.listener(Self::open_settings),
                    ))
                    .child(ui::icon_btn(
                        "close",
                        "✕",
                        false,
                        cx.listener(Self::close_click),
                    )),
            )
    }

    fn query_block(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let query = if self.query.is_empty() {
            if self.editing {
                "│".to_string()
            } else {
                "Type or select text".to_string()
            }
        } else if self.editing {
            format!("{}│", self.query)
        } else {
            self.query.clone()
        };
        let empty = self.query.is_empty();
        let detected = if self.recording {
            "Listening…".to_string()
        } else if self.status.as_ref() == "Transcribing…" {
            "Transcribing…".to_string()
        } else if self.source_lang == "auto" && self.detected != "auto" {
            format!("Detected {}", translate::language_label(&self.detected))
        } else if self.status.as_ref() == "Translating…" {
            "Translating…".to_string()
        } else {
            String::new()
        };

        div()
            .px_4()
            .pt_1()
            .pb_2()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .id("query")
                    .cursor_text()
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::focus_query))
                    .text_xl()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(if empty { theme::muted() } else { theme::text() })
                    .child(query),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .child(ui::icon_btn(
                                "speak-query",
                                if self.speaking == Some(SpeakTarget::Query) {
                                    "⏹"
                                } else {
                                    "🔊"
                                },
                                self.speaking == Some(SpeakTarget::Query),
                                cx.listener(Self::speak_query),
                            ))
                            .child(ui::icon_btn(
                                "mic",
                                if self.recording { "●" } else { "🎤" },
                                self.recording,
                                cx.listener(Self::toggle_mic),
                            ))
                            .child(ui::icon_btn(
                                "copy-query",
                                "⧉",
                                false,
                                cx.listener(Self::copy_query),
                            ))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme::link())
                                    .child(detected),
                            ),
                    )
                    .child(ui::icon_btn(
                        "clear",
                        "✕",
                        false,
                        cx.listener(Self::clear_query),
                    )),
            )
    }

    fn lang_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let src_code = self.display_source_code().to_string();
        let src_auto = self.source_lang == "auto";
        let tgt = self.target_lang.clone();

        div()
            .mx_3()
            .mt_1()
            .px_2()
            .py_1()
            .rounded_md()
            .bg(theme::bar())
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(ui::lang_pill(
                "src-lang",
                self.picker == Picker::Source,
                cx.listener(|this, _, _, cx| this.toggle_picker(Picker::Source, cx)),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme::muted())
                    .child(if src_auto { "▾ Auto" } else { "▾" }),
            )
            .child(
                div()
                    .text_sm()
                    .child(format!(
                        "{} {}",
                        translate::language_short(&src_code),
                        translate::language_flag(&src_code)
                    )),
            ))
            .child(ui::icon_btn(
                "swap",
                "⇌",
                false,
                cx.listener(Self::swap_langs),
            ))
            .child(ui::lang_pill(
                "tgt-lang",
                self.picker == Picker::Target,
                cx.listener(|this, _, _, cx| this.toggle_picker(Picker::Target, cx)),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme::muted())
                    .child("▾"),
            )
            .child(
                div().text_sm().child(format!(
                    "{} {}",
                    translate::language_short(&tgt),
                    translate::language_flag(&tgt)
                )),
            ))
    }

    fn picker_panel(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let side = self.picker;
        if side == Picker::None {
            return None;
        }
        let current = match side {
            Picker::Source => self.source_lang.as_str(),
            Picker::Target => self.target_lang.as_str(),
            Picker::None => "",
        };
        let langs: &[&str] = match side {
            Picker::Source => translate::source_language_cycle(),
            _ => translate::language_cycle(),
        };
        let mut chips = Vec::new();
        for (i, code) in langs.iter().enumerate() {
            let selected = current == *code;
            let code_owned = (*code).to_string();
            chips.push(
                ui::chip(
                    ("pick", i),
                    format!(
                        "{}  {}",
                        translate::language_flag(code),
                        translate::language_short(code)
                    ),
                    selected,
                    cx.listener(move |this, _, _, cx| {
                        this.pick_lang(side, &code_owned, cx);
                    }),
                )
                .into_any_element(),
            );
        }
        Some(
            div()
                .mx_3()
                .mt_2()
                .p_2()
                .rounded_lg()
                .bg(theme::card())
                .border_1()
                .border_color(theme::border())
                .flex()
                .flex_row()
                .flex_wrap()
                .gap_2()
                .children(chips),
        )
    }

    fn results_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut cards = Vec::new();
        if self.results.is_empty() && self.status.as_ref() == "Translating…" {
            cards.push(
                div()
                    .mx_3()
                    .mt_2()
                    .child(ui::banner(ui::BannerKind::Info, "Translating…"))
                    .into_any_element(),
            );
        }
        for (index, result) in self.results.iter().enumerate() {
            let open = !self.collapsed.iter().any(|p| p == result.provider);
            let (body, ok) = match &result.output {
                Ok(text) => (text.clone(), true),
                Err(err) => (err.clone(), false),
            };
            let provider = result.provider;
            let copy = cx.listener(move |this, ev, window, cx| {
                this.copy_result(index, ev, window, cx);
            });
            let speak = cx.listener(move |this, ev, window, cx| {
                this.speak_result(index, ev, window, cx);
            });
            let speaking_this = self.speaking == Some(SpeakTarget::Result(index));
            let toggle = cx.listener(move |this, _, _, cx| {
                this.toggle_card(provider, cx);
            });
            let (primary, extra) = split_translation(&body);
            let wordlike = self.query.chars().count() <= 32 && !self.query.contains('\n');

            cards.push(
                div()
                    .mx_3()
                    .mt_2()
                    .flex()
                    .flex_col()
                    .rounded_lg()
                    .bg(theme::card())
                    .border_1()
                    .border_color(theme::border())
                    .child(
                        div()
                            .id(("hdr", index))
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .px_3()
                            .py_2()
                            .cursor_pointer()
                            .on_mouse_up(MouseButton::Left, toggle)
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_2()
                                    .child(ui::provider_mark(provider))
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(gpui::FontWeight::MEDIUM)
                                            .child(provider.to_string()),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme::muted())
                                    .child(if open { "▾" } else { "▸" }),
                            ),
                    )
                    .when(open, |el| {
                        el.child(
                            div()
                                .px_3()
                                .pb_2()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .when(wordlike && ok, |el| {
                                    el.child(
                                        div()
                                            .text_lg()
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .child(self.query.clone()),
                                    )
                                })
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(if ok {
                                            theme::text()
                                        } else {
                                            theme::danger()
                                        })
                                        .child(primary.to_string()),
                                )
                                .when(!extra.is_empty() && ok, |el| {
                                    el.child(
                                        div()
                                            .text_sm()
                                            .text_color(theme::muted())
                                            .child(extra.to_string()),
                                    )
                                }),
                        )
                        .when(ok, |el| {
                            el.child(
                                div()
                                    .px_3()
                                    .pb_2()
                                    .flex()
                                    .flex_row()
                                    .gap_2()
                                    .child(ui::icon_btn(
                                        ("speak", index),
                                        if speaking_this { "⏹" } else { "🔊" },
                                        speaking_this,
                                        speak,
                                    ))
                                    .child(ui::icon_btn(
                                        ("copy", index),
                                        "⧉",
                                        false,
                                        copy,
                                    )),
                            )
                        })
                    })
                    .into_any_element(),
            );
        }
        div().flex().flex_col().pb_3().children(cards)
    }
}

fn split_translation(text: &str) -> (&str, &str) {
    match text.find('\n') {
        Some(i) => (text[..i].trim(), text[i + 1..].trim()),
        None => (text.trim(), ""),
    }
}
