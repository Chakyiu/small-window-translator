use anyhow::{Context, Result, bail};
use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
#[cfg(target_os = "macos")]
use std::sync::OnceLock;

static PLAYBACK: Mutex<Option<Child>> = Mutex::new(None);

const MAX_CHARS: usize = 2000;

pub fn speak(text: &str, lang: &str) -> Result<()> {
    stop();
    let spoken = trim_speech(text);
    if spoken.is_empty() {
        bail!("Nothing to speak");
    }
    let child = spawn_speech(spoken, lang)?;
    *PLAYBACK.lock().unwrap_or_else(|e| e.into_inner()) = Some(child);
    Ok(())
}

pub fn stop() {
    let mut slot = PLAYBACK.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(mut child) = slot.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

pub fn is_speaking() -> bool {
    let mut slot = PLAYBACK.lock().unwrap_or_else(|e| e.into_inner());
    let Some(child) = slot.as_mut() else {
        return false;
    };
    match child.try_wait() {
        Ok(Some(_)) => {
            *slot = None;
            false
        }
        Ok(None) => true,
        Err(_) => {
            *slot = None;
            false
        }
    }
}

fn trim_speech(text: &str) -> &str {
    let text = text.trim();
    if text.chars().count() <= MAX_CHARS {
        return text;
    }
    let end = text.char_indices().nth(MAX_CHARS).map(|(i, _)| i).unwrap_or(text.len());
    text[..end].trim()
}

fn spawn_speech(text: &str, lang: &str) -> Result<Child> {
    #[cfg(target_os = "macos")]
    {
        return spawn_macos(text, lang);
    }
    #[cfg(target_os = "windows")]
    {
        return spawn_windows(text, lang);
    }
    #[cfg(target_os = "linux")]
    {
        return spawn_linux(text, lang);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = (text, lang);
        bail!("Text-to-speech is not available on this OS");
    }
}

#[cfg(target_os = "macos")]
fn spawn_macos(text: &str, lang: &str) -> Result<Child> {
    let mut cmd = Command::new("say");
    if let Some(voice) = macos_voice_for(lang) {
        cmd.arg("-v").arg(voice);
    }
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("macOS `say` is not available")?;
    write_stdin(&mut child, text)?;
    Ok(child)
}

#[cfg(target_os = "macos")]
fn macos_voice_for(lang: &str) -> Option<String> {
    let wanted = locale_prefix(lang)?;
    macos_voices()
        .iter()
        .find(|voice| {
            voice.locale.starts_with(&wanted)
                || (wanted.ends_with('_') && voice.locale.starts_with(wanted.trim_end_matches('_')))
        })
        .map(|voice| voice.name.clone())
}

#[derive(Debug, Clone)]
struct Voice {
    name: String,
    locale: String,
}

#[cfg(target_os = "macos")]
fn macos_voices() -> &'static [Voice] {
    static VOICES: OnceLock<Vec<Voice>> = OnceLock::new();
    VOICES.get_or_init(|| {
        let output = Command::new("say").args(["-v", "?"]).output().ok();
        let raw = output
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .unwrap_or_default();
        parse_say_voices(&raw)
    })
}

fn parse_say_voices(raw: &str) -> Vec<Voice> {
    raw.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            let (name, rest) = line.split_once(char::is_whitespace)?;
            let locale = rest.split_whitespace().next()?;
            Some(Voice {
                name: name.to_string(),
                locale: locale.to_string(),
            })
        })
        .collect()
}

fn locale_prefix(lang: &str) -> Option<String> {
    Some(
        match lang {
            "en" => "en_",
            "zh" => "zh_CN",
            "zh-tw" => "zh_TW",
            "ja" => "ja_",
            "ko" => "ko_",
            "fr" => "fr_",
            "de" => "de_",
            "es" => "es_",
            "pt" => "pt_",
            "ru" => "ru_",
            "it" => "it_",
            "vi" => "vi_",
            "th" => "th_",
            "id" => "id_",
            "ar" => "ar_",
            _ => return None,
        }
        .to_string(),
    )
}

#[cfg(target_os = "windows")]
fn spawn_windows(text: &str, lang: &str) -> Result<Child> {
    let culture = windows_culture(lang);
    let script = format!(
        "Add-Type -AssemblyName System.Speech; \
         $s = New-Object System.Speech.Synthesis.SpeechSynthesizer; \
         try {{ $s.SelectVoiceByHints('NotSet','NotSet',0,[Globalization.CultureInfo]::GetCultureInfo('{culture}')) }} catch {{}}; \
         $s.Speak([Console]::In.ReadToEnd())"
    );
    let mut child = Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("Windows PowerShell TTS is not available")?;
    write_stdin(&mut child, text)?;
    Ok(child)
}

#[cfg(target_os = "windows")]
fn windows_culture(lang: &str) -> &'static str {
    match lang {
        "en" => "en-US",
        "zh" => "zh-CN",
        "zh-tw" => "zh-TW",
        "ja" => "ja-JP",
        "ko" => "ko-KR",
        "fr" => "fr-FR",
        "de" => "de-DE",
        "es" => "es-ES",
        "pt" => "pt-BR",
        "ru" => "ru-RU",
        "it" => "it-IT",
        "vi" => "vi-VN",
        "th" => "th-TH",
        "id" => "id-ID",
        "ar" => "ar-SA",
        _ => "en-US",
    }
}

#[cfg(target_os = "linux")]
fn spawn_linux(text: &str, lang: &str) -> Result<Child> {
    let voice = linux_voice(lang);
    if let Ok(mut child) = Command::new("espeak-ng")
        .args(["-v", voice])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        write_stdin(&mut child, text)?;
        return Ok(child);
    }
    if let Ok(mut child) = Command::new("espeak")
        .args(["-v", voice])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        write_stdin(&mut child, text)?;
        return Ok(child);
    }
    let mut child = Command::new("spd-say")
        .args(["-w", "-t", "female1"])
        .arg(text)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("Install espeak-ng or speech-dispatcher for text-to-speech")?;
    let _ = &mut child;
    Ok(child)
}

#[cfg(target_os = "linux")]
fn linux_voice(lang: &str) -> &'static str {
    match lang {
        "zh" | "zh-tw" => "zh",
        "ja" => "ja",
        "ko" => "ko",
        "fr" => "fr",
        "de" => "de",
        "es" => "es",
        "pt" => "pt",
        "ru" => "ru",
        "it" => "it",
        "vi" => "vi",
        "th" => "th",
        "id" => "id",
        "ar" => "ar",
        _ => "en",
    }
}

fn write_stdin(child: &mut Child, text: &str) -> Result<()> {
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(text.as_bytes())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_say_voice_list() {
        let raw = "\
Alex                en_US    # Most people recognize me by my voice.
Tingting            zh_CN    # ...
Sinji               zh_TW    # ...
Kyoko               ja_JP    # ...
";
        let voices = parse_say_voices(raw);
        assert_eq!(voices[0].name, "Alex");
        assert_eq!(voices[1].locale, "zh_CN");
        assert_eq!(voices[2].locale, "zh_TW");
    }

    #[test]
    fn locale_prefixes() {
        assert_eq!(locale_prefix("zh").as_deref(), Some("zh_CN"));
        assert_eq!(locale_prefix("zh-tw").as_deref(), Some("zh_TW"));
        assert_eq!(locale_prefix("en").as_deref(), Some("en_"));
        assert_eq!(locale_prefix("auto"), None);
    }

    #[test]
    fn trims_long_speech() {
        let long = "ab".repeat(2000);
        assert!(trim_speech(&long).chars().count() <= MAX_CHARS);
        assert_eq!(trim_speech("  hi  "), "hi");
    }
}
