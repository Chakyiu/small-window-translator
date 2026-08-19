use super::{TranslateRequest, Translator};
use anyhow::{Context, Result, bail};

/// Unofficial `translate.googleapis.com` gtx client. Isolated so it can be
/// removed if Google changes or blocks the endpoint. Default-off in config.
pub struct GoogleUnofficial;

impl Translator for GoogleUnofficial {
    fn id(&self) -> &'static str {
        "Google"
    }

    fn translate(&self, req: &TranslateRequest) -> Result<String> {
        let sl = if req.source_lang == "auto" {
            "auto"
        } else {
            google_lang(&req.source_lang)
        };
        let tl = google_lang(&req.target_lang);
        let q = urlencoding::encode(&req.text);
        let url = format!(
            "https://translate.googleapis.com/translate_a/single?client=gtx&sl={sl}&tl={tl}&dt=t&q={q}"
        );
        let client = super::http_client()?;
        let response = client
            .get(url)
            .send()
            .context("Google unofficial request failed")?;
        let status = response.status();
        let raw = response.text().unwrap_or_default();
        if !status.is_success() {
            bail!("Google HTTP {status}: {raw}");
        }
        parse_gtx_response(&raw)
    }
}

pub fn parse_gtx_response(raw: &str) -> Result<String> {
    let value: serde_json::Value = serde_json::from_str(raw).context("Google JSON")?;
    let Some(chunks) = value.get(0).and_then(|v| v.as_array()) else {
        bail!("unexpected Google response shape");
    };
    let mut out = String::new();
    for chunk in chunks {
        if let Some(text) = chunk.get(0).and_then(|v| v.as_str()) {
            out.push_str(text);
        }
    }
    if out.trim().is_empty() {
        bail!("Google returned empty text");
    }
    Ok(out)
}

fn google_lang(code: &str) -> &str {
    match code {
        "zh" | "zh-cn" => "zh-CN",
        "zh-tw" => "zh-TW",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_gtx_array() {
        let raw = r#"[[["Hello world","你好世界",null,null,10]],null,"zh"]"#;
        assert_eq!(parse_gtx_response(raw).unwrap(), "Hello world");
    }
}
