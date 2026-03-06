use crate::types::*;

// ---------------------------------------------------------------------------
// Provider trait
// ---------------------------------------------------------------------------

pub trait AiProvider: Send + Sync {
    /// Simple completion — no tools, returns text only.
    fn chat(&self, messages: &[Message], system: &str) -> Result<String, AiError>;

    /// Completion with tool use — returns full response with possible tool_use blocks.
    fn chat_with_tools(
        &self,
        messages: &[Message],
        system: &str,
        tools: &[ToolDefinition],
    ) -> Result<AssistantResponse, AiError>;
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum AiError {
    /// HTTP/network error
    Network(String),
    /// API returned an error (status code + body)
    Api { status: u16, body: String },
    /// Response parsing failed
    Parse(String),
    /// Invalid configuration
    Config(String),
}

impl std::fmt::Display for AiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AiError::Network(e) => write!(f, "network error: {e}"),
            AiError::Api { status, body } => write!(f, "API error {status}: {body}"),
            AiError::Parse(e) => write!(f, "parse error: {e}"),
            AiError::Config(e) => write!(f, "config error: {e}"),
        }
    }
}

impl std::error::Error for AiError {}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

pub fn create_provider(config: &AiConfig) -> Result<Box<dyn AiProvider>, AiError> {
    match config.provider.as_str() {
        "anthropic" => Ok(Box::new(crate::anthropic::AnthropicProvider::new(config)?)),
        "openai" => Ok(Box::new(crate::openai::OpenAiProvider::new(config)?)),
        other => Err(AiError::Config(format!(
            "unknown provider: '{other}'. Supported: anthropic, openai"
        ))),
    }
}
