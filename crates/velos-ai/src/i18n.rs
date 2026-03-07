use std::collections::HashMap;
use std::sync::OnceLock;

pub struct I18n {
    messages: &'static HashMap<&'static str, &'static str>,
}

impl I18n {
    pub fn new(lang: &str) -> Self {
        let messages = match lang {
            "ru" => ru(),
            _ => en(),
        };
        Self { messages }
    }

    pub fn get<'a>(&self, key: &'a str) -> &'a str {
        self.messages.get(key).copied().unwrap_or_else(move || {
            // Fallback to English
            en().get(key).copied().unwrap_or(key)
        })
    }
}

fn en() -> &'static HashMap<&'static str, &'static str> {
    static EN: OnceLock<HashMap<&str, &str>> = OnceLock::new();
    EN.get_or_init(|| {
        let mut m = HashMap::new();
        // Crash notification
        m.insert("crash.title", "Process Crashed");
        m.insert("crash.name", "Name");
        m.insert("crash.exit_code", "Exit code");
        m.insert("crash.host", "Host");
        m.insert("crash.time", "Time");
        m.insert("crash.logs", "Last logs");
        m.insert("crash.analysis_header", "AI Analysis");
        m.insert("crash.no_analysis", "AI analysis not configured. Set up with: velos config set ai.provider anthropic");
        m.insert("crash.btn_fix", "Fix");
        m.insert("crash.btn_ignore", "Ignore");
        // Fix flow
        m.insert("fix.started", "Starting AI fix...");
        m.insert("fix.completed", "Fix applied!");
        m.insert("fix.failed", "Fix failed");
        m.insert("fix.changes_summary", "Changes summary");
        m.insert("fix.no_crash_record", "Crash record not found");
        m.insert("fix.iterations", "iterations");
        m.insert("fix.tool_calls", "tool calls");
        m.insert("fix.tokens", "tokens");
        // Error detection (runtime, no crash)
        m.insert("error.title", "Error Detected");
        // Config
        m.insert("config.no_ai", "AI not configured");
        m
    })
}

fn ru() -> &'static HashMap<&'static str, &'static str> {
    static RU: OnceLock<HashMap<&str, &str>> = OnceLock::new();
    RU.get_or_init(|| {
        let mut m = HashMap::new();
        // Crash notification
        m.insert("crash.title", "\u{041f}\u{0440}\u{043e}\u{0446}\u{0435}\u{0441}\u{0441} \u{0443}\u{043f}\u{0430}\u{043b}");
        m.insert("crash.name", "\u{0418}\u{043c}\u{044f}");
        m.insert("crash.exit_code", "\u{041a}\u{043e}\u{0434} \u{0432}\u{044b}\u{0445}\u{043e}\u{0434}\u{0430}");
        m.insert("crash.host", "\u{0425}\u{043e}\u{0441}\u{0442}");
        m.insert("crash.time", "\u{0412}\u{0440}\u{0435}\u{043c}\u{044f}");
        m.insert("crash.logs", "\u{041f}\u{043e}\u{0441}\u{043b}\u{0435}\u{0434}\u{043d}\u{0438}\u{0435} \u{043b}\u{043e}\u{0433}\u{0438}");
        m.insert("crash.analysis_header", "AI-\u{0430}\u{043d}\u{0430}\u{043b}\u{0438}\u{0437}");
        m.insert("crash.no_analysis", "AI-\u{0430}\u{043d}\u{0430}\u{043b}\u{0438}\u{0437} \u{043d}\u{0435} \u{043d}\u{0430}\u{0441}\u{0442}\u{0440}\u{043e}\u{0435}\u{043d}. \u{041d}\u{0430}\u{0441}\u{0442}\u{0440}\u{043e}\u{0439}\u{0442}\u{0435}: velos config set ai.provider anthropic");
        m.insert("crash.btn_fix", "\u{0418}\u{0441}\u{043f}\u{0440}\u{0430}\u{0432}\u{0438}\u{0442}\u{044c}");
        m.insert("crash.btn_ignore", "\u{0418}\u{0433}\u{043d}\u{043e}\u{0440}\u{0438}\u{0440}\u{043e}\u{0432}\u{0430}\u{0442}\u{044c}");
        // Fix flow
        m.insert("fix.started", "\u{0417}\u{0430}\u{043f}\u{0443}\u{0441}\u{043a}\u{0430}\u{044e} AI-\u{0438}\u{0441}\u{043f}\u{0440}\u{0430}\u{0432}\u{043b}\u{0435}\u{043d}\u{0438}\u{0435}...");
        m.insert("fix.completed", "\u{0418}\u{0441}\u{043f}\u{0440}\u{0430}\u{0432}\u{043b}\u{0435}\u{043d}\u{0438}\u{0435} \u{043f}\u{0440}\u{0438}\u{043c}\u{0435}\u{043d}\u{0435}\u{043d}\u{043e}!");
        m.insert("fix.failed", "\u{041d}\u{0435} \u{0443}\u{0434}\u{0430}\u{043b}\u{043e}\u{0441}\u{044c} \u{0438}\u{0441}\u{043f}\u{0440}\u{0430}\u{0432}\u{0438}\u{0442}\u{044c}");
        m.insert("fix.changes_summary", "\u{0421}\u{0432}\u{043e}\u{0434}\u{043a}\u{0430} \u{0438}\u{0437}\u{043c}\u{0435}\u{043d}\u{0435}\u{043d}\u{0438}\u{0439}");
        m.insert("fix.no_crash_record", "\u{0417}\u{0430}\u{043f}\u{0438}\u{0441}\u{044c} \u{043e} \u{043a}\u{0440}\u{0430}\u{0448}\u{0435} \u{043d}\u{0435} \u{043d}\u{0430}\u{0439}\u{0434}\u{0435}\u{043d}\u{0430}");
        m.insert("fix.iterations", "\u{0438}\u{0442}\u{0435}\u{0440}\u{0430}\u{0446}\u{0438}\u{0439}");
        m.insert("fix.tool_calls", "\u{0432}\u{044b}\u{0437}\u{043e}\u{0432}\u{043e}\u{0432} \u{0438}\u{043d}\u{0441}\u{0442}\u{0440}\u{0443}\u{043c}\u{0435}\u{043d}\u{0442}\u{043e}\u{0432}");
        m.insert("fix.tokens", "\u{0442}\u{043e}\u{043a}\u{0435}\u{043d}\u{043e}\u{0432}");
        // Error detection (runtime, no crash)
        m.insert("error.title", "\u{041e}\u{0448}\u{0438}\u{0431}\u{043a}\u{0430} \u{0432} \u{043f}\u{0440}\u{043e}\u{0446}\u{0435}\u{0441}\u{0441}\u{0435}");
        // Config
        m.insert("config.no_ai", "AI \u{043d}\u{0435} \u{043d}\u{0430}\u{0441}\u{0442}\u{0440}\u{043e}\u{0435}\u{043d}");
        m
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_default() {
        let i18n = I18n::new("en");
        assert_eq!(i18n.get("crash.title"), "Process Crashed");
        assert_eq!(i18n.get("crash.btn_fix"), "Fix");
    }

    #[test]
    fn russian() {
        let i18n = I18n::new("ru");
        assert!(i18n.get("crash.title").contains('\u{0443}'));
        assert!(i18n.get("crash.btn_fix").contains('\u{0418}'));
    }

    #[test]
    fn unknown_lang_falls_back_to_english() {
        let i18n = I18n::new("xx");
        assert_eq!(i18n.get("crash.title"), "Process Crashed");
    }

    #[test]
    fn unknown_key_returns_key() {
        let i18n = I18n::new("en");
        assert_eq!(i18n.get("nonexistent.key"), "nonexistent.key");
    }
}
