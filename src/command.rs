#[derive(Debug, Clone)]
pub enum AppCommand {
    TranslateSelection,
    OpenSettings,
    OpenVocab,
    Quit,
    ReloadConfig,
    CloseEmbeddedSettings,
    CloseEmbeddedVocab,
    Retranslate {
        text: String,
        source_lang: String,
        target_lang: String,
    },
}
