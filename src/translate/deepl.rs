use super::{TranslateRequest, Translator};
use anyhow::{Context, Result, bail};
use serde::Deserialize;

pub struct DeepL {
    pub api_key: String,
    pub use_pro: bool,
}

impl Translator for DeepL {
    fn id(&self) -> &'static str {
        "DeepL"
    }

    fn translate(&self, req: &TranslateRequest) -> Result<String> {
        let host = if self.use_pro {
            "https://api.deepl.com/v2/translate"
        } else {
            "https://api-free.deepl.com/v2/translate"
        };
        let target = deepl_lang(&req.target_lang);
        let client = super::http_client()?;
        let mut form = vec![
            ("text", req.text.as_str()),
            ("target_lang", target),
        ];
        if req.source_lang != "auto" {
            form.push(("source_lang", deepl_lang(&req.source_lang)));
        }
        let response = client
            .post(host)
            .header("Authorization", format!("DeepL-Auth-Key {}", self.api_key.trim()))
            .form(&form)
            .send()
            .context("DeepL request failed")?;
        let status = response.status();
        let body = response.text().unwrap_or_default();
        if !status.is_success() {
            bail!("DeepL HTTP {status}: {body}");
        }
        let parsed: DeepLResponse = serde_json::from_str(&body).context("DeepL JSON")?;
        let text = parsed
            .translations
            .into_iter()
            .map(|t| t.text)
            .collect::<Vec<_>>()
            .join("\n");
        if text.is_empty() {
            bail!("DeepL returned empty text");
        }
        Ok(text)
    }
}

#[derive(Deserialize)]
struct DeepLResponse {
    translations: Vec<DeepLTranslation>,
}

#[derive(Deserialize)]
struct DeepLTranslation {
    text: String,
}

fn deepl_lang(code: &str) -> &'static str {
    match code.to_ascii_lowercase().as_str() {
        "en" => "EN",
        "zh" | "zh-cn" => "ZH",
        "zh-tw" => "ZH",
        "ja" => "JA",
        "ko" => "KO",
        "fr" => "FR",
        "de" => "DE",
        "es" => "ES",
        "pt" => "PT",
        "ru" => "RU",
        "it" => "IT",
        "id" => "ID",
        "ar" => "AR",
        other if other.len() == 2 => Box::leak(other.to_ascii_uppercase().into_boxed_str()),
        _ => "EN",
    }
}
