use super::{TranslateRequest, Translator};
use anyhow::{bail, Context, Result};
use serde_json::Value;

/// Unofficial `dict.youdao.com` dictionary/translate client. Isolated so it
/// can be removed if Youdao changes or blocks the endpoint.
pub struct YoudaoDict;

impl Translator for YoudaoDict {
    fn id(&self) -> &'static str {
        "Youdao"
    }

    fn translate(&self, req: &TranslateRequest) -> Result<String> {
        let q = urlencoding::encode(req.text.trim());
        let mut url = format!("https://dict.youdao.com/jsonapi_s?doctype=json&jsonversion=4&q={q}");
        if let Some(le) = youdao_le(&req.source_lang) {
            url.push_str("&le=");
            url.push_str(le);
        }
        let client = super::http_client()?;
        let response = client.get(url).send().context("Youdao request failed")?;
        let status = response.status();
        let raw = response.text().unwrap_or_default();
        if !status.is_success() {
            bail!("Youdao HTTP {status}: {raw}");
        }
        format_youdao(&raw)
    }
}

pub fn format_youdao(raw: &str) -> Result<String> {
    let value: Value = serde_json::from_str(raw).context("Youdao JSON")?;
    let phone = phonetics(&value);
    let mut defs = Vec::new();
    push_ec(&value, &mut defs);
    push_ce(&value, &mut defs);
    push_newjc(&value, &mut defs);

    if defs.is_empty() {
        let input = value.get("input").and_then(Value::as_str).unwrap_or("");
        if let Some(tran) = value
            .pointer("/fanyi/tran")
            .and_then(Value::as_str)
            .map(clean)
        {
            if !tran.is_empty() && !same_text(&tran, input) {
                defs.push(tran);
            }
        }
    }
    if defs.is_empty() {
        if let Some(web) = first_web_trans(&value) {
            defs.push(web);
        }
    }

    let mut lines = Vec::new();
    if let Some(first) = defs.first() {
        lines.push(first.clone());
    }
    if let Some(phone) = phone {
        lines.push(phone);
    }
    if defs.len() > 1 {
        lines.extend(defs.into_iter().skip(1));
    }
    let out = lines
        .into_iter()
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    if out.trim().is_empty() {
        bail!("Youdao returned no definition");
    }
    Ok(out)
}

fn phonetics(value: &Value) -> Option<String> {
    let word = value
        .pointer("/ec/word")
        .or_else(|| value.pointer("/simple/word/0"))
        .or_else(|| value.pointer("/simple/word"));
    let word = word?;
    let uk = word.get("ukphone").and_then(Value::as_str).unwrap_or("");
    let us = word.get("usphone").and_then(Value::as_str).unwrap_or("");
    let phone = word.get("phone").and_then(Value::as_str).unwrap_or("");
    let mut parts = Vec::new();
    if !uk.is_empty() {
        parts.push(format!("UK /{uk}/"));
    }
    if !us.is_empty() {
        parts.push(format!("US /{us}/"));
    }
    if parts.is_empty() && !phone.is_empty() {
        parts.push(format!("/{phone}/"));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("  "))
    }
}

fn push_ec(value: &Value, lines: &mut Vec<String>) {
    let word = value.pointer("/ec/word");
    let Some(word) = word else { return };
    let trs = match word.get("trs") {
        Some(Value::Array(items)) => items.as_slice(),
        _ => &[],
    };
    for tr in trs {
        let pos = tr.get("pos").and_then(Value::as_str).unwrap_or("").trim();
        let tran = tr
            .get("tran")
            .and_then(Value::as_str)
            .map(clean)
            .unwrap_or_default();
        if tran.is_empty() {
            continue;
        }
        if pos.is_empty() {
            lines.push(tran);
        } else {
            lines.push(format!("{pos} {tran}"));
        }
    }
}

fn push_ce(value: &Value, lines: &mut Vec<String>) {
    let trs = match value.pointer("/ce/word/trs") {
        Some(Value::Array(items)) => items,
        _ => return,
    };
    for tr in trs {
        let text = tr
            .get("#text")
            .and_then(Value::as_str)
            .map(clean)
            .unwrap_or_default();
        if !text.is_empty() {
            lines.push(text);
        }
    }
}

fn push_newjc(value: &Value, lines: &mut Vec<String>) {
    let word = value
        .pointer("/newjc/word")
        .or_else(|| value.pointer("/jc/word"));
    let Some(word) = word else { return };
    if let Some(head) = word.get("head") {
        let hw = head.get("hw").and_then(Value::as_str).unwrap_or("");
        let pjm = head.get("pjm").and_then(Value::as_str).unwrap_or("");
        let rs = head.get("rs").and_then(Value::as_str).unwrap_or("");
        let mut head_line = String::new();
        if !hw.is_empty() {
            head_line.push_str(hw);
        }
        if !pjm.is_empty() && pjm != hw {
            if !head_line.is_empty() {
                head_line.push(' ');
            }
            head_line.push_str(pjm);
        }
        if !rs.is_empty() {
            if !head_line.is_empty() {
                head_line.push(' ');
            }
            head_line.push_str(&format!("/{rs}/"));
        }
        if !head_line.is_empty() {
            lines.push(head_line);
        }
    }
    if let Some(Value::Array(senses)) = word.get("sense") {
        for sense in senses.iter().take(3) {
            if let Some(Value::Array(phrs)) = sense.get("phrList") {
                for phr in phrs.iter().take(2) {
                    if let Some(text) = phr.get("jmsyT").and_then(Value::as_str) {
                        let text = clean(text);
                        if !text.is_empty() {
                            lines.push(text);
                        }
                    }
                    if let Some(text) = phr.get("zhuyi").and_then(Value::as_str) {
                        let text = clean(text);
                        if !text.is_empty() {
                            lines.push(text);
                        }
                    }
                }
            }
        }
    }
}

fn first_web_trans(value: &Value) -> Option<String> {
    if let Some(Value::Array(web)) = value.pointer("/ec/web_trans") {
        if let Some(first) = web.iter().find_map(Value::as_str).map(clean) {
            if !first.is_empty() {
                return Some(first);
            }
        }
    }
    let items = value.pointer("/web_trans/web-translation")?.as_array()?;
    for item in items {
        let trans = match item.get("trans") {
            Some(Value::Array(t)) => t,
            _ => continue,
        };
        for t in trans {
            if let Some(v) = t.get("value").and_then(Value::as_str).map(clean) {
                if !v.is_empty() {
                    return Some(v);
                }
            }
        }
    }
    None
}

fn clean(s: &str) -> String {
    strip_tags(s)
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .trim()
        .to_string()
}

fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

fn same_text(a: &str, b: &str) -> bool {
    a.chars()
        .filter(|c| !c.is_whitespace())
        .eq(b.chars().filter(|c| !c.is_whitespace()))
}

fn youdao_le(source: &str) -> Option<&'static str> {
    Some(match source {
        "en" => "en",
        "zh" | "zh-cn" | "zh-tw" => "zh",
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
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_english_entry() {
        let raw = r#"{
            "ec": {
                "web_trans": ["您好", "哈啰"],
                "word": {
                    "usphone": "həˈloʊ",
                    "ukphone": "həˈləʊ",
                    "trs": [
                        {"pos": "int.", "tran": "喂，你好"},
                        {"pos": "n.", "tran": "招呼，问候"}
                    ]
                }
            }
        }"#;
        let out = format_youdao(raw).unwrap();
        assert!(out.contains("UK /həˈləʊ/"));
        assert!(out.contains("int. 喂，你好"));
        assert!(out.contains("n. 招呼，问候"));
    }

    #[test]
    fn formats_chinese_entry() {
        let raw = r##"{
            "ce": {
                "word": {
                    "trs": [{"#text": "apple", "#tran": "苹果；"}]
                }
            }
        }"##;
        assert_eq!(format_youdao(raw).unwrap(), "apple");
    }

    #[test]
    fn formats_sentence() {
        let raw = r#"{
            "input": "This is a test sentence.",
            "fanyi": {"tran": "这是一个测试句子。"}
        }"#;
        assert_eq!(format_youdao(raw).unwrap(), "这是一个测试句子。");
    }

    #[test]
    fn strips_html() {
        assert_eq!(strip_tags("<b>hello</b> there"), "hello there");
    }
}
