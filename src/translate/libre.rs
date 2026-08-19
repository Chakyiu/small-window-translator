use super::{TranslateRequest, Translator};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

pub struct LibreTranslate {
    pub endpoint: String,
    pub api_key: String,
}

impl Translator for LibreTranslate {
    fn id(&self) -> &'static str {
        "LibreTranslate"
    }

    fn translate(&self, req: &TranslateRequest) -> Result<String> {
        let base = self.endpoint.trim_end_matches('/');
        let url = format!("{base}/translate");
        let body = LibreBody {
            q: req.text.clone(),
            source: if req.source_lang == "auto" {
                "auto".to_string()
            } else {
                libre_lang(&req.source_lang).to_string()
            },
            target: libre_lang(&req.target_lang).to_string(),
            format: "text",
            api_key: if self.api_key.trim().is_empty() {
                None
            } else {
                Some(self.api_key.trim().to_string())
            },
        };
        let client = super::http_client()?;
        let response = client
            .post(url)
            .json(&body)
            .send()
            .context("LibreTranslate request failed")?;
        let status = response.status();
        let raw = response.text().unwrap_or_default();
        if !status.is_success() {
            bail!("LibreTranslate HTTP {status}: {raw}");
        }
        let parsed: LibreResponse = serde_json::from_str(&raw).context("LibreTranslate JSON")?;
        if parsed.translated_text.trim().is_empty() {
            bail!("LibreTranslate returned empty text");
        }
        Ok(parsed.translated_text)
    }
}

#[derive(Serialize)]
struct LibreBody {
    q: String,
    source: String,
    target: String,
    format: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    api_key: Option<String>,
}

#[derive(Deserialize)]
struct LibreResponse {
    translated_text: String,
}

fn libre_lang(code: &str) -> &str {
    match code {
        "zh" | "zh-cn" => "zh",
        "zh-tw" => "zt",
        other => other,
    }
}
