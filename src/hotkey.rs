use anyhow::{Result, bail};
use global_hotkey::hotkey::{Code, HotKey, Modifiers};

pub fn parse_hotkey(spec: &str) -> Result<HotKey> {
    let mut modifiers = Modifiers::empty();
    let mut code = None;

    for raw in spec.split('+') {
        let part = raw.trim();
        if part.is_empty() {
            continue;
        }
        match part.to_ascii_lowercase().as_str() {
            "alt" | "option" | "opt" => modifiers |= Modifiers::ALT,
            "ctrl" | "control" => modifiers |= Modifiers::CONTROL,
            "shift" => modifiers |= Modifiers::SHIFT,
            "super" | "cmd" | "command" | "meta" | "win" | "windows" => {
                modifiers |= Modifiers::SUPER;
            }
            key => {
                if code.is_some() {
                    bail!("too many keys in hotkey `{spec}`");
                }
                code = Some(parse_code(key)?);
            }
        }
    }

    let Some(code) = code else {
        bail!("hotkey `{spec}` is missing a key");
    };
    let mods = if modifiers.is_empty() {
        None
    } else {
        Some(modifiers)
    };
    Ok(HotKey::new(mods, code))
}

fn parse_code(key: &str) -> Result<Code> {
    let k = key.to_ascii_lowercase();
    let code = match k.as_str() {
        "a" | "keya" => Code::KeyA,
        "b" | "keyb" => Code::KeyB,
        "c" | "keyc" => Code::KeyC,
        "d" | "keyd" => Code::KeyD,
        "e" | "keye" => Code::KeyE,
        "f" | "keyf" => Code::KeyF,
        "g" | "keyg" => Code::KeyG,
        "h" | "keyh" => Code::KeyH,
        "i" | "keyi" => Code::KeyI,
        "j" | "keyj" => Code::KeyJ,
        "k" | "keyk" => Code::KeyK,
        "l" | "keyl" => Code::KeyL,
        "m" | "keym" => Code::KeyM,
        "n" | "keyn" => Code::KeyN,
        "o" | "keyo" => Code::KeyO,
        "p" | "keyp" => Code::KeyP,
        "q" | "keyq" => Code::KeyQ,
        "r" | "keyr" => Code::KeyR,
        "s" | "keys" => Code::KeyS,
        "t" | "keyt" => Code::KeyT,
        "u" | "keyu" => Code::KeyU,
        "v" | "keyv" => Code::KeyV,
        "w" | "keyw" => Code::KeyW,
        "x" | "keyx" => Code::KeyX,
        "y" | "keyy" => Code::KeyY,
        "z" | "keyz" => Code::KeyZ,
        "0" | "digit0" => Code::Digit0,
        "1" | "digit1" => Code::Digit1,
        "2" | "digit2" => Code::Digit2,
        "3" | "digit3" => Code::Digit3,
        "4" | "digit4" => Code::Digit4,
        "5" | "digit5" => Code::Digit5,
        "6" | "digit6" => Code::Digit6,
        "7" | "digit7" => Code::Digit7,
        "8" | "digit8" => Code::Digit8,
        "9" | "digit9" => Code::Digit9,
        "f1" => Code::F1,
        "f2" => Code::F2,
        "f3" => Code::F3,
        "f4" => Code::F4,
        "f5" => Code::F5,
        "f6" => Code::F6,
        "f7" => Code::F7,
        "f8" => Code::F8,
        "f9" => Code::F9,
        "f10" => Code::F10,
        "f11" => Code::F11,
        "f12" => Code::F12,
        "space" => Code::Space,
        "enter" | "return" => Code::Enter,
        other => bail!("unsupported hotkey key `{other}`"),
    };
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_alt_d() {
        let hotkey = parse_hotkey("Alt+D").unwrap();
        assert_eq!(hotkey.key, Code::KeyD);
        assert!(hotkey.mods.contains(Modifiers::ALT));
    }

    #[test]
    fn parses_ctrl_shift_t() {
        let hotkey = parse_hotkey("ctrl+shift+t").unwrap();
        assert_eq!(hotkey.key, Code::KeyT);
        assert!(hotkey.mods.contains(Modifiers::CONTROL | Modifiers::SHIFT));
    }

    #[test]
    fn rejects_empty() {
        assert!(parse_hotkey("Alt").is_err());
    }
}
