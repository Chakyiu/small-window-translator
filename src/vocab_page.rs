use crate::theme;
use crate::translate;
use crate::ui;
use crate::vocab::{self, Word};
use crate::AppCommand;
use gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, KeyBinding, KeyDownEvent, SharedString,
    Window, actions, div, prelude::*, px,
};
use std::sync::mpsc::Sender;

actions!(vocab_page, [CloseVocab]);

pub enum VocabEvent {
    Dismiss,
}

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", CloseVocab, Some("Vocab"))]);
}

pub struct VocabView {
    focus: FocusHandle,
    field_focus: FocusHandle,
    editing: bool,
    words: Vec<Word>,
    filter: String,
    review_i: Option<usize>,
    review_revealed: bool,
    status: SharedString,
    tx: Sender<AppCommand>,
    embedded: bool,
}

impl EventEmitter<VocabEvent> for VocabView {}

impl VocabView {
    pub fn new(cx: &mut Context<Self>, tx: Sender<AppCommand>) -> Self {
        Self::create(cx, tx, false)
    }

    pub fn embedded(cx: &mut Context<Self>, tx: Sender<AppCommand>) -> Self {
        Self::create(cx, tx, true)
    }

    fn create(cx: &mut Context<Self>, tx: Sender<AppCommand>, embedded: bool) -> Self {
        let words = vocab::list().unwrap_or_default();
        let status = format!("{} saved", words.len()).into();
        Self {
            focus: cx.focus_handle(),
            field_focus: cx.focus_handle(),
            editing: true,
            words,
            filter: String::new(),
            review_i: None,
            review_revealed: false,
            status,
            tx,
            embedded,
        }
    }

    fn close(&mut self, _: &CloseVocab, window: &mut Window, cx: &mut Context<Self>) {
        if self.review_i.is_some() {
            self.review_i = None;
            self.review_revealed = false;
            cx.notify();
            return;
        }
        if self.embedded {
            let _ = self.tx.send(AppCommand::CloseEmbeddedVocab);
            cx.emit(VocabEvent::Dismiss);
            return;
        }
        window.remove_window();
    }

    fn close_click(&mut self, ev: &gpui::MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.review_i = None;
        self.review_revealed = false;
        self.close(&CloseVocab, window, cx);
        let _ = ev;
    }

    fn back_to_query(
        &mut self,
        _: &gpui::MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.review_i = None;
        self.review_revealed = false;
        self.close(&CloseVocab, window, cx);
    }

    fn reload(&mut self) {
        match vocab::list() {
            Ok(words) => {
                self.words = words;
                self.status = format!("{} saved", self.words.len()).into();
            }
            Err(err) => {
                self.status = format!("Words: {err}").into();
            }
        }
    }

    fn filtered(&self) -> Vec<&Word> {
        let q = self.filter.trim().to_lowercase();
        self.words
            .iter()
            .filter(|w| {
                q.is_empty()
                    || w.word.to_lowercase().contains(&q)
                    || w.translation.to_lowercase().contains(&q)
            })
            .collect()
    }

    fn focus_search(
        &mut self,
        _: &gpui::MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.editing = true;
        window.focus(&self.field_focus);
        cx.notify();
    }

    fn start_review(&mut self, _: &gpui::MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.filtered().is_empty() {
            self.status = "No words to review".into();
            cx.notify();
            return;
        }
        self.review_i = Some(0);
        self.review_revealed = false;
        self.editing = false;
        self.status = "Review".into();
        cx.notify();
    }

    fn exit_review(&mut self, _: &gpui::MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.review_i = None;
        self.review_revealed = false;
        self.editing = true;
        cx.notify();
    }

    fn reveal_review(&mut self, _: &gpui::MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.review_revealed = true;
        cx.notify();
    }

    fn next_review(&mut self, _: &gpui::MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        let filtered: Vec<i64> = self.filtered().into_iter().map(|w| w.id).collect();
        let Some(i) = self.review_i else {
            return;
        };
        if let Some(id) = filtered.get(i) {
            if let Err(err) = vocab::mark_reviewed(*id) {
                self.status = format!("Review: {err}").into();
            }
        }
        let next = i + 1;
        if next >= filtered.len() {
            self.review_i = None;
            self.review_revealed = false;
            self.editing = true;
            self.reload();
            self.status = "Review finished".into();
        } else {
            self.review_i = Some(next);
            self.review_revealed = false;
            self.status = format!("Review {}/{}", next + 1, filtered.len()).into();
        }
        cx.notify();
    }

    fn delete_word(&mut self, id: i64, cx: &mut Context<Self>) {
        match vocab::delete(id) {
            Ok(()) => {
                if let Some(i) = self.review_i {
                    let n = self.filtered().len().saturating_sub(1);
                    if n == 0 {
                        self.review_i = None;
                        self.review_revealed = false;
                        self.editing = true;
                    } else if i >= n {
                        self.review_i = Some(n - 1);
                        self.review_revealed = false;
                    }
                }
                self.reload();
            }
            Err(err) => {
                self.status = format!("Delete failed: {err}").into();
            }
        }
        cx.notify();
    }

    fn on_key(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = ev.keystroke.key.as_str();
        if key == "escape" {
            self.close(&CloseVocab, window, cx);
            return;
        }
        if self.review_i.is_some() || !self.editing {
            return;
        }
        if key == "backspace" || key == "delete" {
            self.filter.pop();
            cx.notify();
            return;
        }
        if let Some(ch) = ev.keystroke.key_char.as_ref() {
            if ch.chars().all(|c| !c.is_control()) {
                self.filter.push_str(ch);
                cx.notify();
            }
        }
    }
}

impl Focusable for VocabView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for VocabView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        ui::page()
            .id("vocab")
            .key_context("Vocab")
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::close))
            .on_key_down(cx.listener(Self::on_key))
            .flex()
            .flex_col()
            .child(self.header(cx))
            .child(self.body(cx))
            .child(self.footer())
    }
}

impl VocabView {
    fn header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .justify_between()
            .items_center()
            .px_4()
            .pt_3()
            .pb_2()
            .border_b_1()
            .border_color(theme::border())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(ui::heading("Saved words"))
                    .child(ui::subtitle("Local vocabulary")),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_1()
                    .when(self.embedded, |el| {
                        el.child(ui::ghost_button(
                            "vocab-back",
                            "← Query",
                            cx.listener(Self::back_to_query),
                        ))
                    })
                    .child(ui::icon_btn(
                        "vocab-close",
                        "✕",
                        false,
                        cx.listener(Self::close_click),
                    )),
            )
    }

    fn body(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let content = if let Some(i) = self.review_i {
            self.review_body(i, cx)
        } else {
            self.list_body(cx)
        };
        div()
            .id("vocab-body")
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.))
            .p_4()
            .gap_3()
            .overflow_y_scroll()
            .child(content)
    }

    fn list_body(&mut self, cx: &mut Context<Self>) -> gpui::Div {
        let filter = self.filter.clone();
        let filtered: Vec<Word> = self.filtered().into_iter().cloned().collect();
        let total = self.words.len();
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                ui::card()
                    .child(ui::field(
                        "vocab-search",
                        "Search",
                        filter,
                        "Filter saved words",
                        self.editing && self.review_i.is_none(),
                        cx.listener(Self::focus_search),
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .child(ui::primary_button(
                                "start-review",
                                "Review",
                                cx.listener(Self::start_review),
                            ))
                            .child(ui::subtitle(format!(
                                "{} shown · {} saved",
                                filtered.len(),
                                total
                            ))),
                    ),
            )
            .when(filtered.is_empty(), |el| {
                el.child(ui::banner(
                    ui::BannerKind::Info,
                    if total == 0 {
                        "No saved words yet. Translate something, then press ★."
                    } else {
                        "No matches."
                    },
                ))
            })
            .children(filtered.into_iter().map(|word| {
                let id = word.id;
                let langs = format!(
                    "{} → {}",
                    translate::language_short(&word.source_lang),
                    translate::language_short(&word.target_lang)
                );
                let preview = first_line(&word.translation);
                ui::card()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_between()
                            .items_start()
                            .gap_2()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .flex_1()
                                    .min_w(px(0.))
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .child(word.word.clone()),
                                    )
                                    .when(!preview.is_empty(), |el| {
                                        el.child(ui::subtitle(preview))
                                    })
                                    .child(ui::subtitle(if word.provider.is_empty() {
                                        langs
                                    } else {
                                        format!("{} · {}", word.provider, langs)
                                    })),
                            )
                            .child(ui::icon_btn(
                                ("del-word", id as usize),
                                "✕",
                                false,
                                cx.listener(move |this, _, _, cx| this.delete_word(id, cx)),
                            )),
                    )
            }))
    }

    fn review_body(&mut self, index: usize, cx: &mut Context<Self>) -> gpui::Div {
        let filtered: Vec<Word> = self.filtered().into_iter().cloned().collect();
        let total = filtered.len();
        let Some(word) = filtered.get(index).cloned() else {
            return div()
                .child(ui::heading("Review"))
                .child(ui::banner(ui::BannerKind::Info, "Nothing to review."));
        };
        let id = word.id;
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(ui::heading("Review"))
            .child(ui::subtitle(format!("{}/{}", index + 1, total)))
            .child(
                ui::card()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(word.word.clone()),
                    )
                    .child(ui::subtitle(format!(
                        "{} → {}",
                        translate::language_label(&word.source_lang),
                        translate::language_label(&word.target_lang)
                    )))
                    .when(self.review_revealed, |el| {
                        el.child(div().text_sm().child(if word.translation.is_empty() {
                            "(no translation saved)".to_string()
                        } else {
                            word.translation.clone()
                        }))
                    })
                    .when(!self.review_revealed, |el| {
                        el.child(ui::subtitle("Tap Show to reveal the translation."))
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap_2()
                    .when(!self.review_revealed, |el| {
                        el.child(ui::primary_button(
                            "reveal-review",
                            "Show",
                            cx.listener(Self::reveal_review),
                        ))
                    })
                    .child(ui::ghost_button(
                        "next-review",
                        if index + 1 >= total { "Done" } else { "Next" },
                        cx.listener(Self::next_review),
                    ))
                    .child(ui::ghost_button(
                        "delete-review",
                        "Delete",
                        cx.listener(move |this, _, _, cx| this.delete_word(id, cx)),
                    ))
                    .child(ui::ghost_button(
                        "exit-review",
                        "Back to list",
                        cx.listener(Self::exit_review),
                    )),
            )
    }

    fn footer(&self) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .items_center()
            .px_4()
            .py_3()
            .border_t_1()
            .border_color(theme::border())
            .bg(theme::card())
            .child(ui::subtitle(self.status.clone().to_string()))
    }
}

fn first_line(text: &str) -> String {
    text.lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim()
        .to_string()
}
