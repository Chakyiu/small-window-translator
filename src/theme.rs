use gpui::{Rgba, rgb};

pub fn bg() -> Rgba {
    rgb(0x1a1a1c)
}
pub fn card() -> Rgba {
    rgb(0x2a2a2c)
}
pub fn border() -> Rgba {
    rgb(0x3a3a3c)
}
pub fn text() -> Rgba {
    rgb(0xf5f5f7)
}
pub fn muted() -> Rgba {
    rgb(0x8e8e93)
}
pub fn accent() -> Rgba {
    rgb(0x0a84ff)
}
pub fn link() -> Rgba {
    rgb(0x64d2ff)
}
pub fn danger() -> Rgba {
    rgb(0xff453a)
}
pub fn ok() -> Rgba {
    rgb(0x30d158)
}
pub fn field() -> Rgba {
    rgb(0x3a3a3c)
}
pub fn bar() -> Rgba {
    rgb(0x141416)
}

pub fn provider_color(name: &str) -> Rgba {
    match name {
        "DeepL" => rgb(0x0f2b46),
        "OpenAI" => rgb(0x10a37f),
        "LibreTranslate" => rgb(0xe85d04),
        "Google" => rgb(0x4285f4),
        _ => rgb(0x636366),
    }
}

pub const POPUP_WIDTH: f32 = 400.0;
pub const POPUP_HEIGHT: f32 = 580.0;
pub const SETTINGS_WIDTH: f32 = 760.0;
pub const SETTINGS_HEIGHT: f32 = 560.0;
pub const SETTINGS_SIDEBAR: f32 = 168.0;
