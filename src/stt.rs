use crate::config::OpenAiConfig;
use anyhow::{Context, Result, bail};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};
use serde::Deserialize;
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub const MAX_SECONDS: u64 = 12;

pub struct Session {
    stop: Arc<AtomicBool>,
    samples: Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
    stream: Option<cpal::Stream>,
}

impl Session {
    pub fn start() -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .context("No microphone found")?;
        let supported = device
            .default_input_config()
            .context("Could not read microphone format")?;
        let sample_rate = supported.sample_rate().0;
        let channels = supported.channels() as usize;
        let stop = Arc::new(AtomicBool::new(false));
        let samples = Arc::new(Mutex::new(Vec::new()));
        let stream_config = supported.config();

        let stream = match supported.sample_format() {
            SampleFormat::F32 => {
                build_stream::<f32>(&device, &stream_config, channels, &stop, &samples)?
            }
            SampleFormat::I16 => {
                build_stream::<i16>(&device, &stream_config, channels, &stop, &samples)?
            }
            SampleFormat::I32 => {
                build_stream::<i32>(&device, &stream_config, channels, &stop, &samples)?
            }
            SampleFormat::U16 => {
                build_stream::<u16>(&device, &stream_config, channels, &stop, &samples)?
            }
            other => bail!("Unsupported microphone format: {other}"),
        };
        stream.play().context("Could not start microphone")?;

        Ok(Self {
            stop,
            samples,
            sample_rate,
            stream: Some(stream),
        })
    }

    pub fn finish(mut self) -> Result<Vec<u8>> {
        self.stop.store(true, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(40));
        self.stream.take();
        let samples = self
            .samples
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        if samples.len() < (self.sample_rate as usize / 8).max(1) {
            bail!("Recording too short — click the mic, speak, then click again");
        }
        encode_wav(&samples, self.sample_rate)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.stream.take();
    }
}

pub fn stt_ready(cfg: &OpenAiConfig) -> bool {
    !cfg.api_key.trim().is_empty() || !cfg.base_url.contains("api.openai.com")
}

pub fn transcribe(cfg: &OpenAiConfig, wav: &[u8], source_lang: &str) -> Result<String> {
    let base = cfg.base_url.trim_end_matches('/');
    let url = format!("{base}/audio/transcriptions");
    let model = if cfg.whisper_model.trim().is_empty() {
        "whisper-1"
    } else {
        cfg.whisper_model.trim()
    };

    let mut form = reqwest::blocking::multipart::Form::new()
        .text("model", model.to_string())
        .part(
            "file",
            reqwest::blocking::multipart::Part::bytes(wav.to_vec())
                .file_name("speech.wav")
                .mime_str("audio/wav")?,
        );
    if let Some(lang) = whisper_lang(source_lang) {
        form = form.text("language", lang);
    }

    let response = crate::translate::http_client()?
        .post(url)
        .bearer_auth(cfg.api_key.trim())
        .multipart(form)
        .send()
        .context("speech-to-text request failed")?;
    let status = response.status();
    let raw = response.text().unwrap_or_default();
    if !status.is_success() {
        bail!("STT HTTP {status}: {raw}");
    }
    let parsed: Transcript = serde_json::from_str(&raw).context("STT JSON")?;
    let text = parsed.text.trim().to_string();
    if text.is_empty() {
        bail!("No speech detected");
    }
    Ok(text)
}

fn whisper_lang(code: &str) -> Option<String> {
    match code {
        "" | "auto" => None,
        "zh-tw" => Some("zh".into()),
        other => Some(other.to_string()),
    }
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    stop: &Arc<AtomicBool>,
    samples: &Arc<Mutex<Vec<f32>>>,
) -> Result<cpal::Stream>
where
    T: SizedSample + Send + 'static,
    f32: FromSample<T>,
{
    let stop = stop.clone();
    let samples = samples.clone();
    device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                if stop.load(Ordering::Relaxed) {
                    return;
                }
                let mut buf = samples.lock().unwrap_or_else(|e| e.into_inner());
                if channels <= 1 {
                    buf.extend(data.iter().copied().map(|s| s.to_sample::<f32>()));
                } else {
                    for frame in data.chunks(channels) {
                        let sum: f32 = frame.iter().copied().map(|s| s.to_sample::<f32>()).sum();
                        buf.push(sum / channels as f32);
                    }
                }
            },
            |err| eprintln!("swtrans: microphone error: {err}"),
            None,
        )
        .context("Could not open microphone")
}

fn encode_wav(samples: &[f32], sample_rate: u32) -> Result<Vec<u8>> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(&mut cursor, spec)?;
        for sample in samples {
            let amp = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
            writer.write_sample(amp)?;
        }
        writer.finalize()?;
    }
    Ok(cursor.into_inner())
}

#[derive(Deserialize)]
struct Transcript {
    text: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_has_header() {
        let wav = encode_wav(&[0.0, 0.5, -0.5], 16_000).unwrap();
        assert!(wav.starts_with(b"RIFF"));
        assert!(wav.len() > 44);
    }

    #[test]
    fn whisper_lang_maps() {
        assert_eq!(whisper_lang("auto"), None);
        assert_eq!(whisper_lang("zh-tw").as_deref(), Some("zh"));
        assert_eq!(whisper_lang("en").as_deref(), Some("en"));
    }

    #[test]
    fn parses_transcript() {
        let parsed: Transcript = serde_json::from_str(r#"{"text":" hello "}"#).unwrap();
        assert_eq!(parsed.text.trim(), "hello");
    }
}
