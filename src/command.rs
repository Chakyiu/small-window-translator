#[derive(Debug, Clone)]
pub enum AppCommand {
    TranslateSelection,
    OpenSettings,
    Quit,
    ReloadConfig,
    Retranslate {
        text: String,
        source_lang: String,
        target_lang: String,
    },
}
