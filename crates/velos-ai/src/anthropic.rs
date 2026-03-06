use serde_json::{json, Value};

use crate::provider::{AiError, AiProvider};
use crate::types::*;

const DEFAULT_API_URL: &str = "https://api.anthropic.com";
const API_VERSION: &str = "2023-06-01";
const MAX_TOKENS: u32 = 16384;

pub struct AnthropicProvider {
    api_key: String,
    model: String,
    base_url: String,
}

impl AnthropicProvider {
    pub fn new(config: &AiConfig) -> Result<Self, AiError> {
        if config.api_key.is_empty() {
            return Err(AiError::Config("ai.api_key is required".into()));
        }
        if config.model.is_empty() {
            return Err(AiError::Config("ai.model is required".into()));
        }
        let base_url = if config.base_url.is_empty() {
            DEFAULT_API_URL.to_string()
        } else {
            config.base_url.trim_end_matches('/').to_string()
        };
        Ok(Self {
            api_key: config.api_key.clone(),
            model: config.model.clone(),
            base_url,
        })
    }

    fn build_messages(&self, messages: &[Message]) -> Vec<Value> {
        let mut result = Vec::new();

        for msg in messages {
            match msg.role {
                Role::User => {
                    let blocks = self.content_to_json(&msg.content);
                    result.push(json!({ "role": "user", "content": blocks }));
                }
                Role::Assistant => {
                    let blocks = self.content_to_json(&msg.content);
                    result.push(json!({ "role": "assistant", "content": blocks }));
                }
                Role::Tool => {
                    // Tool results go as user messages with tool_result content blocks
                    let blocks = self.content_to_json(&msg.content);
                    result.push(json!({ "role": "user", "content": blocks }));
                }
            }
        }
        result
    }

    fn content_to_json(&self, content: &[ContentBlock]) -> Vec<Value> {
        content
            .iter()
            .map(|b| match b {
                ContentBlock::Text { text } => json!({ "type": "text", "text": text }),
                ContentBlock::ToolUse { id, name, input } => {
                    json!({ "type": "tool_use", "id": id, "name": name, "input": input })
                }
                ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                } => {
                    json!({
                        "type": "tool_result",
                        "tool_use_id": tool_use_id,
                        "content": content,
                        "is_error": is_error,
                    })
                }
            })
            .collect()
    }

    fn build_tools(&self, tools: &[ToolDefinition]) -> Vec<Value> {
        tools
            .iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.parameters,
                })
            })
            .collect()
    }

    fn parse_response(&self, body: Value) -> Result<AssistantResponse, AiError> {
        let content_arr = body["content"]
            .as_array()
            .ok_or_else(|| AiError::Parse("missing 'content' array".into()))?;

        let mut content = Vec::new();
        for block in content_arr {
            let block_type = block["type"]
                .as_str()
                .unwrap_or("");
            match block_type {
                "text" => {
                    let text = block["text"].as_str().unwrap_or("").to_string();
                    content.push(ContentBlock::Text { text });
                }
                "tool_use" => {
                    let id = block["id"].as_str().unwrap_or("").to_string();
                    let name = block["name"].as_str().unwrap_or("").to_string();
                    let input = block["input"].clone();
                    content.push(ContentBlock::ToolUse { id, name, input });
                }
                _ => {} // ignore unknown block types
            }
        }

        let stop_reason = match body["stop_reason"].as_str().unwrap_or("end_turn") {
            "tool_use" => StopReason::ToolUse,
            "max_tokens" => StopReason::MaxTokens,
            _ => StopReason::EndTurn,
        };

        let usage = Usage {
            input_tokens: body["usage"]["input_tokens"].as_u64().unwrap_or(0) as u32,
            output_tokens: body["usage"]["output_tokens"].as_u64().unwrap_or(0) as u32,
        };

        Ok(AssistantResponse {
            content,
            stop_reason,
            usage,
        })
    }

    fn do_request(&self, body: Value) -> Result<Value, AiError> {
        let url = format!("{}/v1/messages", self.base_url);

        let agent = ureq::Agent::new_with_config(
            ureq::config::Config::builder()
                .timeout_global(Some(std::time::Duration::from_secs(300)))
                .build(),
        );

        let resp = agent.post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", API_VERSION)
            .header("content-type", "application/json")
            .send_json(&body)
            .map_err(|e| {
                let msg = e.to_string();
                if msg.contains("status: 4") || msg.contains("status: 5") {
                    AiError::Api {
                        status: extract_status(&msg),
                        body: msg,
                    }
                } else {
                    AiError::Network(msg)
                }
            })?;

        let body: Value = resp
            .into_body()
            .read_json()
            .map_err(|e| AiError::Parse(format!("failed to read response body: {e}")))?;

        if let Some(err) = body.get("error") {
            return Err(AiError::Api {
                status: 400,
                body: err.to_string(),
            });
        }

        Ok(body)
    }
}

impl AiProvider for AnthropicProvider {
    fn chat(&self, messages: &[Message], system: &str) -> Result<String, AiError> {
        let body = json!({
            "model": self.model,
            "system": system,
            "messages": self.build_messages(messages),
            "max_tokens": MAX_TOKENS,
        });

        let resp = self.do_request(body)?;
        let parsed = self.parse_response(resp)?;
        Ok(parsed.text())
    }

    fn chat_with_tools(
        &self,
        messages: &[Message],
        system: &str,
        tools: &[ToolDefinition],
    ) -> Result<AssistantResponse, AiError> {
        let mut body = json!({
            "model": self.model,
            "system": system,
            "messages": self.build_messages(messages),
            "max_tokens": MAX_TOKENS,
        });

        if !tools.is_empty() {
            body["tools"] = json!(self.build_tools(tools));
        }

        let resp = self.do_request(body)?;
        self.parse_response(resp)
    }
}

fn extract_status(msg: &str) -> u16 {
    // Try to extract status code from error message like "http status: 401"
    if let Some(pos) = msg.find("status: ") {
        let num_str = &msg[pos + 8..];
        if let Ok(n) = num_str[..3.min(num_str.len())].trim().parse::<u16>() {
            return n;
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_messages_user_and_assistant() {
        let config = AiConfig {
            provider: "anthropic".into(),
            model: "claude-sonnet-4-5".into(),
            api_key: "test".into(),
            base_url: String::new(),
            max_iterations: 30,
            auto_analyze: true,
            auto_fix: false,
        };
        let provider = AnthropicProvider::new(&config).unwrap();

        let messages = vec![
            Message::user("Hello"),
            Message::assistant("Hi there"),
        ];
        let result = provider.build_messages(&messages);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0]["role"], "user");
        assert_eq!(result[1]["role"], "assistant");
    }

    #[test]
    fn build_messages_tool_result() {
        let config = AiConfig {
            provider: "anthropic".into(),
            model: "test".into(),
            api_key: "test".into(),
            base_url: String::new(),
            max_iterations: 30,
            auto_analyze: true,
            auto_fix: false,
        };
        let provider = AnthropicProvider::new(&config).unwrap();

        let messages = vec![Message::tool_result("tool-1", "file contents here", false)];
        let result = provider.build_messages(&messages);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["role"], "user");
        assert_eq!(result[0]["content"][0]["type"], "tool_result");
        assert_eq!(result[0]["content"][0]["tool_use_id"], "tool-1");
    }

    #[test]
    fn build_tools_format() {
        let config = AiConfig {
            provider: "anthropic".into(),
            model: "test".into(),
            api_key: "test".into(),
            base_url: String::new(),
            max_iterations: 30,
            auto_analyze: true,
            auto_fix: false,
        };
        let provider = AnthropicProvider::new(&config).unwrap();

        let tools = vec![ToolDefinition {
            name: "read_file".into(),
            description: "Read a file".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" }
                },
                "required": ["path"]
            }),
        }];
        let result = provider.build_tools(&tools);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["name"], "read_file");
        assert!(result[0]["input_schema"].is_object());
    }

    #[test]
    fn parse_text_response() {
        let config = AiConfig {
            provider: "anthropic".into(),
            model: "test".into(),
            api_key: "test".into(),
            base_url: String::new(),
            max_iterations: 30,
            auto_analyze: true,
            auto_fix: false,
        };
        let provider = AnthropicProvider::new(&config).unwrap();

        let body = json!({
            "content": [{"type": "text", "text": "Hello world"}],
            "stop_reason": "end_turn",
            "usage": { "input_tokens": 10, "output_tokens": 5 }
        });
        let resp = provider.parse_response(body).unwrap();
        assert_eq!(resp.text(), "Hello world");
        assert_eq!(resp.stop_reason, StopReason::EndTurn);
        assert_eq!(resp.usage.input_tokens, 10);
    }

    #[test]
    fn parse_tool_use_response() {
        let config = AiConfig {
            provider: "anthropic".into(),
            model: "test".into(),
            api_key: "test".into(),
            base_url: String::new(),
            max_iterations: 30,
            auto_analyze: true,
            auto_fix: false,
        };
        let provider = AnthropicProvider::new(&config).unwrap();

        let body = json!({
            "content": [
                {"type": "text", "text": "Let me read that file."},
                {"type": "tool_use", "id": "tu_1", "name": "read_file", "input": {"path": "/app/src/main.rs"}}
            ],
            "stop_reason": "tool_use",
            "usage": { "input_tokens": 100, "output_tokens": 50 }
        });
        let resp = provider.parse_response(body).unwrap();
        assert_eq!(resp.stop_reason, StopReason::ToolUse);
        assert_eq!(resp.tool_calls().len(), 1);
    }

    #[test]
    fn empty_api_key_returns_error() {
        let config = AiConfig {
            provider: "anthropic".into(),
            model: "test".into(),
            api_key: String::new(),
            base_url: String::new(),
            max_iterations: 30,
            auto_analyze: true,
            auto_fix: false,
        };
        assert!(AnthropicProvider::new(&config).is_err());
    }
}
