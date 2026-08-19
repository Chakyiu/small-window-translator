use crate::config::Config;
use anyhow::Result;
use std::thread;

mod deepl;
mod google;
mod libre;
mod openai;

#[derive(Debug, Clone)]
pub struct TranslateRequest {
    pub text: String,
    pub source_lang: String,
    pub target_lang: String,
}

#[derive(Debug, Clone)]
pub struct TranslateResult {
    pub provider: &'static str,
    pub output: Result<String, String>,
}

pub trait Translator: Send + Sync {
    fn id(&self) -> &'static str;
    fn translate(&self, req: &TranslateRequest) -> Result<String>;
}

pub fn language_short(code: &str) -> &'static str {
    match code {
        "en" => "EN",
        "zh" => "简中",
        "zh-tw" => "繁中",
        "ja" => "JA",
        "ko" => "KO",
        "fr" => "FR",
        "de" => "DE",
        "es" => "ES",
        "pt" => "PT",
        "ru" => "RU",
        "it" => "IT",
        "vi" => "VI",
        "th" => "TH",
        "id" => "ID",
        "ar" => "AR",
        _ => "?",
    }
}

pub fn language_flag(code: &str) -> &'static str {
    match code {
        "auto" => "🌐",
        "en" => "🇬🇧",
        "zh" => "🇨🇳",
        "zh-tw" => "🇭🇰",
        "ja" => "🇯🇵",
        "ko" => "🇰🇷",
        "fr" => "🇫🇷",
        "de" => "🇩🇪",
        "es" => "🇪🇸",
        "pt" => "🇵🇹",
        "ru" => "🇷🇺",
        "it" => "🇮🇹",
        "vi" => "🇻🇳",
        "th" => "🇹🇭",
        "id" => "🇮🇩",
        "ar" => "🇸🇦",
        _ => "🌐",
    }
}

pub fn source_language_cycle() -> &'static [&'static str] {
    &[
        "auto", "en", "zh", "zh-tw", "ja", "ko", "fr", "de", "es", "pt", "ru", "it", "vi", "th",
        "id", "ar",
    ]
}

pub fn language_label(code: &str) -> &'static str {
    match code {
        "auto" => "Auto",
        "en" => "English",
        "zh" => "Chinese (Simplified)",
        "zh-tw" => "Chinese (Traditional)",
        "ja" => "Japanese",
        "ko" => "Korean",
        "fr" => "French",
        "de" => "German",
        "es" => "Spanish",
        "pt" => "Portuguese",
        "ru" => "Russian",
        "it" => "Italian",
        "vi" => "Vietnamese",
        "th" => "Thai",
        "id" => "Indonesian",
        "ar" => "Arabic",
        _ => "Unknown",
    }
}

pub fn language_cycle() -> &'static [&'static str] {
    &[
        "en", "zh", "zh-tw", "ja", "ko", "fr", "de", "es", "pt", "ru", "it", "vi", "th", "id",
        "ar",
    ]
}

pub fn next_language(current: &str) -> &'static str {
    let langs = language_cycle();
    match langs.iter().position(|c| *c == current) {
        Some(i) => langs[(i + 1) % langs.len()],
        None => langs[0],
    }
}

pub fn detect_source(text: &str, configured: &str) -> String {
    if configured != "auto" && !configured.is_empty() {
        return configured.to_string();
    }
    whatlang::detect(text)
        .map(|info| iso_from_whatlang(info.lang()))
        .unwrap_or_else(|| "auto".to_string())
}

fn iso_from_whatlang(lang: whatlang::Lang) -> String {
    use whatlang::Lang::*;
    match lang {
        Eng => "en",
        Cmn => "zh",
        Jpn => "ja",
        Kor => "ko",
        Fra => "fr",
        Deu => "de",
        Spa => "es",
        Por => "pt",
        Rus => "ru",
        Ita => "it",
        Vie => "vi",
        Tha => "th",
        Ind => "id",
        Ara => "ar",
        _ => "auto",
    }
    .to_string()
}

pub fn translate_all(cfg: &Config, text: &str) -> Vec<TranslateResult> {
    let req = TranslateRequest {
        text: text.to_string(),
        source_lang: detect_source(text, &cfg.source_lang),
        target_lang: cfg.target_lang.clone(),
    };

    let mut jobs: Vec<(&'static str, Box<dyn FnOnce() -> Result<String> + Send>)> = Vec::new();

    if cfg.deepl.enabled && !cfg.deepl.api_key.trim().is_empty() {
        let translator = deepl::DeepL {
            api_key: cfg.deepl.api_key.clone(),
            use_pro: cfg.deepl.use_pro,
        };
        let req = req.clone();
        jobs.push((translator.id(), Box::new(move || translator.translate(&req))));
    }
    if cfg.openai.enabled && !cfg.openai.api_key.trim().is_empty() {
        let translator = openai::OpenAiCompat {
            api_key: cfg.openai.api_key.clone(),
            base_url: cfg.openai.base_url.clone(),
            model: cfg.openai.model.clone(),
        };
        let req = req.clone();
        jobs.push((translator.id(), Box::new(move || translator.translate(&req))));
    }
    if cfg.libre.enabled && !cfg.libre.endpoint.trim().is_empty() {
        let translator = libre::LibreTranslate {
            endpoint: cfg.libre.endpoint.clone(),
            api_key: cfg.libre.api_key.clone(),
        };
        let req = req.clone();
        jobs.push((translator.id(), Box::new(move || translator.translate(&req))));
    }
    if cfg.google.enabled {
        let translator = google::GoogleUnofficial;
        let req = req.clone();
        jobs.push((translator.id(), Box::new(move || translator.translate(&req))));
    }

    if jobs.is_empty() {
        return vec![TranslateResult {
            provider: "setup",
            output: Err(
                "No provider is ready. Open Settings, add an API key or enable Google (off by default)."
                    .into(),
            ),
        }];
    }

    let mut handles = Vec::new();
    for (id, job) in jobs {
        handles.push((
            id,
            thread::spawn(move || job().map_err(|e| e.to_string())),
        ));
    }

    handles
        .into_iter()
        .map(|(provider, handle)| TranslateResult {
            provider,
            output: handle.join().unwrap_or_else(|_| Err("worker panicked".into())),
        })
        .collect()
}

pub(crate) fn http_client() -> Result<reqwest::blocking::Client> {
    Ok(reqwest::blocking::Client::builder()
        .user_agent("swtrans/0.1")
        .timeout(std::time::Duration::from_secs(20))
        .build()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycles_languages() {
        assert_eq!(next_language("en"), "zh");
        assert_eq!(next_language("ar"), "en");
    }

    #[test]
    fn detect_english() {
        let lang = detect_source("This is a simple English sentence.", "auto");
        assert_eq!(lang, "en");
    }

    #[test]
    fn flags_for_known_langs() {
        assert_eq!(language_flag("en"), "🇬🇧");
        assert_eq!(language_flag("zh-tw"), "🇭🇰");
        assert_eq!(language_flag("auto"), "🌐");
    }
}
