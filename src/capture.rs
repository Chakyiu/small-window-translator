use anyhow::Result;
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, Default)]
pub struct ScreenRect {
    pub x: f64,
    pub y: f64,
    #[allow(dead_code)]
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Default)]
pub struct Selection {
    pub text: String,
    pub bounds: Option<ScreenRect>,
}

/// Read the current selection. Accessibility / UIA / AT-SPI first, then a
/// clipboard snapshot + simulated copy + restore fallback.
pub fn capture_selection() -> Selection {
    if let Some(selection) = try_accessibility() {
        if !selection.text.trim().is_empty() {
            return selection;
        }
    }
    match simulate_copy_restore() {
        Ok(text) if !text.trim().is_empty() => Selection {
            text,
            bounds: None,
        },
        _ => Selection::default(),
    }
}

pub fn cursor_position() -> Option<(f64, f64)> {
    use enigo::{Enigo, Mouse, Settings};
    let enigo = Enigo::new(&Settings::default()).ok()?;
    let (x, y) = enigo.location().ok()?;
    Some((x as f64, y as f64))
}

fn try_accessibility() -> Option<Selection> {
    use on_selected_text::{GetTextError, GetTextOptions, get_selected_text_with_options};

    let opts = GetTextOptions::default()
        .without_clipboard_fallback()
        .with_bounds();
    match get_selected_text_with_options(opts) {
        Ok(hit) => Some(Selection {
            text: hit.text,
            bounds: hit.bounds.map(|b| ScreenRect {
                x: b.x as f64,
                y: b.y as f64,
                width: b.width as f64,
                height: b.height as f64,
            }),
        }),
        Err(GetTextError::NoTextSelected) => Some(Selection::default()),
        Err(_) => None,
    }
}

fn simulate_copy_restore() -> Result<String> {
    let mut clipboard = arboard::Clipboard::new()?;
    let previous = clipboard.get_text().ok();

    send_copy()?;

    let mut captured = String::new();
    for _ in 0..20 {
        thread::sleep(Duration::from_millis(20));
        if let Ok(text) = clipboard.get_text() {
            let changed = previous.as_ref().is_none_or(|old| old != &text);
            if changed && !text.trim().is_empty() {
                captured = text;
                break;
            }
        }
    }

    match previous {
        Some(old) => {
            let _ = clipboard.set_text(old);
        }
        None => {
            let _ = clipboard.clear();
        }
    }

    Ok(captured)
}

fn send_copy() -> Result<()> {
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};
    let mut enigo = Enigo::new(&Settings::default())?;
    let modifier = if cfg!(target_os = "macos") {
        Key::Meta
    } else {
        Key::Control
    };
    enigo.key(modifier, Direction::Press)?;
    enigo.key(Key::Unicode('c'), Direction::Click)?;
    enigo.key(modifier, Direction::Release)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_selection_is_empty() {
        let s = Selection::default();
        assert!(s.text.is_empty());
        assert!(s.bounds.is_none());
    }
}
