use crate::theme;
use gpui::{
    App, Div, InteractiveElement, IntoElement, MouseUpEvent, ParentElement, Stateful, Styled,
    Window, div, px,
};

pub fn page() -> Div {
    div()
        .flex()
        .flex_col()
        .size_full()
        .bg(theme::bg())
        .text_color(theme::text())
}

pub fn heading(text: impl Into<String>) -> impl IntoElement {
    div().text_lg().font_weight(gpui::FontWeight::SEMIBOLD).child(text.into())
}

pub fn subtitle(text: impl Into<String>) -> impl IntoElement {
    div().text_sm().text_color(theme::muted()).child(text.into())
}

pub fn label(text: impl Into<String>) -> impl IntoElement {
    div()
        .text_xs()
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(theme::muted())
        .child(text.into())
}

pub fn card() -> Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .p_3()
        .rounded_lg()
        .bg(theme::card())
        .border_1()
        .border_color(theme::border())
}

pub fn banner(kind: BannerKind, text: impl Into<String>) -> impl IntoElement {
    let (bg, fg) = match kind {
        BannerKind::Info => (theme::card(), theme::muted()),
        BannerKind::Warn => (theme::field(), theme::danger()),
        BannerKind::Ok => (theme::card(), theme::ok()),
    };
    div()
        .p_3()
        .rounded_lg()
        .bg(bg)
        .border_1()
        .border_color(theme::border())
        .text_sm()
        .text_color(fg)
        .child(text.into())
}

#[derive(Clone, Copy)]
pub enum BannerKind {
    Info,
    Warn,
    Ok,
}

pub fn primary_button(
    id: impl Into<gpui::ElementId>,
    label: impl Into<String>,
    handler: impl Fn(&MouseUpEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id.into())
        .px_3()
        .py_2()
        .rounded_md()
        .bg(theme::accent())
        .text_color(theme::text())
        .text_sm()
        .cursor_pointer()
        .hover(|s| s.opacity(0.9))
        .on_mouse_up(gpui::MouseButton::Left, handler)
        .child(label.into())
}

pub fn ghost_button(
    id: impl Into<gpui::ElementId>,
    label: impl Into<String>,
    handler: impl Fn(&MouseUpEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id.into())
        .px_3()
        .py_2()
        .rounded_md()
        .bg(theme::card())
        .border_1()
        .border_color(theme::border())
        .text_sm()
        .cursor_pointer()
        .hover(|s| s.bg(theme::field()))
        .on_mouse_up(gpui::MouseButton::Left, handler)
        .child(label.into())
}

pub fn toggle(
    id: impl Into<gpui::ElementId>,
    label: impl Into<String>,
    on: bool,
    handler: impl Fn(&MouseUpEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let knob = if on { "On" } else { "Off" };
    div()
        .id(id.into())
        .flex()
        .flex_row()
        .justify_between()
        .items_center()
        .cursor_pointer()
        .on_mouse_up(gpui::MouseButton::Left, handler)
        .child(div().text_sm().child(label.into()))
        .child(
            div()
                .w(px(44.))
                .h(px(22.))
                .rounded_xl()
                .flex()
                .items_center()
                .justify_center()
                .bg(if on { theme::ok() } else { theme::field() })
                .text_xs()
                .child(knob),
        )
}

pub fn nav_item(
    id: impl Into<gpui::ElementId>,
    label: impl Into<String>,
    selected: bool,
    handler: impl Fn(&MouseUpEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id.into())
        .w_full()
        .px_3()
        .py_2()
        .rounded_md()
        .text_sm()
        .cursor_pointer()
        .bg(if selected { theme::field() } else { theme::bg() })
        .text_color(if selected { theme::text() } else { theme::muted() })
        .hover(|s| s.bg(theme::field()).text_color(theme::text()))
        .on_mouse_up(gpui::MouseButton::Left, handler)
        .child(label.into())
}

pub fn field(
    id: impl Into<gpui::ElementId>,
    title: impl Into<String>,
    value: impl Into<String>,
    placeholder: &str,
    active: bool,
    handler: impl Fn(&MouseUpEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let value = value.into();
    let shown = if value.is_empty() {
        placeholder.to_string()
    } else {
        value
    };
    div()
        .id(id.into())
        .flex()
        .flex_col()
        .gap_1()
        .px_3()
        .py_2()
        .rounded_md()
        .bg(if active { theme::field() } else { theme::bg() })
        .border_1()
        .border_color(if active { theme::accent() } else { theme::border() })
        .cursor_pointer()
        .on_mouse_up(gpui::MouseButton::Left, handler)
        .child(label(title))
        .child(
            div()
                .text_sm()
                .text_color(if shown == placeholder {
                    theme::muted()
                } else {
                    theme::text()
                })
                .child(if active {
                    format!("{shown}│")
                } else {
                    shown
                }),
        )
}

pub fn chip(
    id: impl Into<gpui::ElementId>,
    label: impl Into<String>,
    selected: bool,
    handler: impl Fn(&MouseUpEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id.into())
        .px_2()
        .py_1()
        .rounded_md()
        .text_xs()
        .cursor_pointer()
        .bg(if selected { theme::accent() } else { theme::field() })
        .text_color(theme::text())
        .hover(|s| s.opacity(0.85))
        .on_mouse_up(gpui::MouseButton::Left, handler)
        .child(label.into())
}

#[allow(dead_code)]
pub fn toolbar() -> Div {
    div()
        .flex()
        .flex_row()
        .justify_between()
        .items_center()
        .gap_2()
}

pub fn icon_btn(
    id: impl Into<gpui::ElementId>,
    glyph: impl Into<String>,
    active: bool,
    handler: impl Fn(&MouseUpEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id.into())
        .w(px(26.))
        .h(px(22.))
        .rounded_md()
        .flex()
        .items_center()
        .justify_center()
        .text_sm()
        .cursor_pointer()
        .bg(if active { theme::accent() } else { theme::bg() })
        .text_color(if active { theme::text() } else { theme::muted() })
        .hover(|s| {
            s.bg(if active { theme::accent() } else { theme::field() })
                .text_color(theme::text())
                .opacity(if active { 0.9 } else { 1.0 })
        })
        .on_mouse_up(gpui::MouseButton::Left, handler)
        .child(glyph.into())
}

pub fn provider_mark(name: &str) -> impl IntoElement {
    let letter = name.chars().next().unwrap_or('?').to_string();
    div()
        .w(px(22.))
        .h(px(22.))
        .rounded_md()
        .flex()
        .items_center()
        .justify_center()
        .bg(theme::provider_color(name))
        .text_xs()
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(theme::text())
        .child(letter)
}

pub fn lang_pill(
    id: impl Into<gpui::ElementId>,
    open: bool,
    handler: impl Fn(&MouseUpEvent, &mut Window, &mut App) + 'static,
) -> gpui::Stateful<Div> {
    div()
        .id(id.into())
        .flex()
        .flex_row()
        .items_center()
        .gap_1()
        .px_2()
        .py_1()
        .rounded_md()
        .cursor_pointer()
        .bg(if open { theme::field() } else { theme::bar() })
        .hover(|s| s.bg(theme::field()))
        .on_mouse_up(gpui::MouseButton::Left, handler)
}

#[allow(dead_code)]
pub fn spacer() -> Stateful<Div> {
    div().id("spacer").h(px(4.))
}
