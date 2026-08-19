use super::{TranslateRequest, Translator, language_label};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

pub struct OpenAiCompat {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
}

impl Translator for OpenAiCompat {
    fn id(&self) -> &'static str {
        "OpenAI"
    }

    fn translate(&self, req: &TranslateRequest) -> Result<String> {
        let base = self.base_url.trim_end_matches('/');
        let url = format!("{base}/chat/completions");
        let target = language_label(&req.target_lang);
        let system = format!(
            "You are a translator. Translate the user's text into {target}. Return only the translation, no quotes or explanation."
        );
        let body = ChatRequest {
            model: self.model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system",
                    content: system,
                },
                ChatMessage {
                    role: "user",
                    content: req.text.clone(),
                },
            ],
            temperature: 0.2,
        };
        let client = super::http_client()?;
        let response = client
            .post(url)
            .bearer_auth(self.api_key.trim())
            .json(&body)
            .send()
            .context("OpenAI-compatible request failed")?;
        let status = response.status();
        let raw = response.text().unwrap_or_default();
        if !status.is_success() {
            bail!("OpenAI HTTP {status}: {raw}");
        }
        let parsed: ChatResponse = serde_json::from_str(&raw).context("OpenAI JSON")?;
        let text = parsed
            .choices
            .into_iter()
            .next()
            .and_then(|c| c.message.content)
            .unwrap_or_default()
            .trim()
            .to_string();
        if text.is_empty() {
            bail!("OpenAI returned empty text");
        }
        Ok(text)
    }
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
}

#[derive(Serialize)]
struct ChatMessage {
    role: &'static str,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatReply,
}

#[derive(Deserialize)]
struct ChatReply {
    content: Option<String>,
}
