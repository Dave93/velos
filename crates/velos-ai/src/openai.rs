use serde_json::{json, Value};

use crate::provider::{AiError, AiProvider};
use crate::types::*;

const DEFAULT_API_URL: &str = "https://api.openai.com";
const MAX_TOKENS: u32 = 16384;

pub struct OpenAiProvider {
    api_key: String,
    model: String,
    base_url: String,
}

impl OpenAiProvider {
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

    fn build_messages(&self, messages: &[Message], system: &str) -> Vec<Value> {
        let mut result = Vec::new();

        // System prompt as first message
        if !system.is_empty() {
            result.push(json!({ "role": "system", "content": system }));
        }

        for msg in messages {
            match msg.role {
                Role::User => {
                    // Flatten content blocks to string for simple text messages
                    let text = self.extract_text(&msg.content);
                    result.push(json!({ "role": "user", "content": text }));
                }
                Role::Assistant => {
                    // Check if this has tool calls
                    let tool_calls: Vec<Value> = msg
                        .content
                        .iter()
                        .filter_map(|b| match b {
                            ContentBlock::ToolUse { id, name, input } => Some(json!({
                                "id": id,
                                "type": "function",
                                "function": {
                                    "name": name,
                                    "arguments": input.to_string(),
                                }
                            })),
                            _ => None,
                        })
                        .collect();

                    let text = self.extract_text(&msg.content);

                    if tool_calls.is_empty() {
                        result.push(json!({ "role": "assistant", "content": text }));
                    } else {
                        let mut m = json!({ "role": "assistant" });
                        if !text.is_empty() {
                            m["content"] = json!(text);
                        }
                        m["tool_calls"] = json!(tool_calls);
                        result.push(m);
                    }
                }
                Role::Tool => {
                    // Each tool result is a separate message
                    for block in &msg.content {
                        if let ContentBlock::ToolResult {
                            tool_use_id,
                            content,
                            ..
                        } = block
                        {
                            result.push(json!({
                                "role": "tool",
                                "tool_call_id": tool_use_id,
                                "content": content,
                            }));
                        }
                    }
                }
            }
        }
        result
    }

    fn extract_text(&self, content: &[ContentBlock]) -> String {
        content
            .iter()
            .filter_map(|b| match b {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }

    fn build_tools(&self, tools: &[ToolDefinition]) -> Vec<Value> {
        tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters,
                    }
                })
            })
            .collect()
    }

    fn parse_response(&self, body: Value) -> Result<AssistantResponse, AiError> {
        let choice = body["choices"]
            .as_array()
            .and_then(|arr| arr.first())
            .ok_or_else(|| AiError::Parse("missing 'choices' array".into()))?;

        let message = &choice["message"];
        let mut content = Vec::new();

        // Text content
        if let Some(text) = message["content"].as_str() {
            if !text.is_empty() {
                content.push(ContentBlock::Text {
                    text: text.to_string(),
                });
            }
        }

        // Tool calls
        if let Some(tool_calls) = message["tool_calls"].as_array() {
            for tc in tool_calls {
                let id = tc["id"].as_str().unwrap_or("").to_string();
                let name = tc["function"]["name"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                let args_str = tc["function"]["arguments"]
                    .as_str()
                    .unwrap_or("{}");
                let input: Value =
                    serde_json::from_str(args_str).unwrap_or_else(|_| json!({}));
                content.push(ContentBlock::ToolUse { id, name, input });
            }
        }

        let stop_reason = match choice["finish_reason"].as_str().unwrap_or("stop") {
            "tool_calls" => StopReason::ToolUse,
            "length" => StopReason::MaxTokens,
            _ => StopReason::EndTurn,
        };

        let usage = Usage {
            input_tokens: body["usage"]["prompt_tokens"]
                .as_u64()
                .unwrap_or(0) as u32,
            output_tokens: body["usage"]["completion_tokens"]
                .as_u64()
                .unwrap_or(0) as u32,
        };

        Ok(AssistantResponse {
            content,
            stop_reason,
            usage,
        })
    }

    fn do_request(&self, body: Value) -> Result<Value, AiError> {
        let url = format!("{}/v1/chat/completions", self.base_url);

        let resp = ureq::post(&url)
            .header("Authorization", &format!("Bearer {}", self.api_key))
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

impl AiProvider for OpenAiProvider {
    fn chat(&self, messages: &[Message], system: &str) -> Result<String, AiError> {
        let body = json!({
            "model": self.model,
            "messages": self.build_messages(messages, system),
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
            "messages": self.build_messages(messages, system),
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

    fn test_config() -> AiConfig {
        AiConfig {
            provider: "openai".into(),
            model: "gpt-4o".into(),
            api_key: "test-key".into(),
            base_url: String::new(),
            max_iterations: 30,
            auto_analyze: true,
            auto_fix: false,
        }
    }

    #[test]
    fn build_messages_with_system() {
        let provider = OpenAiProvider::new(&test_config()).unwrap();
        let messages = vec![Message::user("Hello")];
        let result = provider.build_messages(&messages, "You are helpful");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0]["role"], "system");
        assert_eq!(result[0]["content"], "You are helpful");
        assert_eq!(result[1]["role"], "user");
    }

    #[test]
    fn build_messages_assistant_with_tool_calls() {
        let provider = OpenAiProvider::new(&test_config()).unwrap();
        let messages = vec![Message {
            role: Role::Assistant,
            content: vec![
                ContentBlock::Text {
                    text: "Let me check.".into(),
                },
                ContentBlock::ToolUse {
                    id: "call_1".into(),
                    name: "read_file".into(),
                    input: json!({"path": "/app/main.rs"}),
                },
            ],
        }];
        let result = provider.build_messages(&messages, "");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["role"], "assistant");
        assert!(result[0]["tool_calls"].is_array());
        assert_eq!(result[0]["tool_calls"][0]["function"]["name"], "read_file");
    }

    #[test]
    fn build_messages_tool_result() {
        let provider = OpenAiProvider::new(&test_config()).unwrap();
        let messages = vec![Message::tool_result("call_1", "file contents", false)];
        let result = provider.build_messages(&messages, "");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["role"], "tool");
        assert_eq!(result[0]["tool_call_id"], "call_1");
    }

    #[test]
    fn build_tools_format() {
        let provider = OpenAiProvider::new(&test_config()).unwrap();
        let tools = vec![ToolDefinition {
            name: "read_file".into(),
            description: "Read a file".into(),
            parameters: json!({
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"]
            }),
        }];
        let result = provider.build_tools(&tools);
        assert_eq!(result[0]["type"], "function");
        assert_eq!(result[0]["function"]["name"], "read_file");
        assert!(result[0]["function"]["parameters"].is_object());
    }

    #[test]
    fn parse_text_response() {
        let provider = OpenAiProvider::new(&test_config()).unwrap();
        let body = json!({
            "choices": [{
                "message": { "role": "assistant", "content": "Hello!" },
                "finish_reason": "stop"
            }],
            "usage": { "prompt_tokens": 10, "completion_tokens": 5 }
        });
        let resp = provider.parse_response(body).unwrap();
        assert_eq!(resp.text(), "Hello!");
        assert_eq!(resp.stop_reason, StopReason::EndTurn);
        assert_eq!(resp.usage.input_tokens, 10);
    }

    #[test]
    fn parse_tool_calls_response() {
        let provider = OpenAiProvider::new(&test_config()).unwrap();
        let body = json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_abc",
                        "type": "function",
                        "function": {
                            "name": "read_file",
                            "arguments": "{\"path\":\"/app/src/main.rs\"}"
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": { "prompt_tokens": 50, "completion_tokens": 20 }
        });
        let resp = provider.parse_response(body).unwrap();
        assert_eq!(resp.stop_reason, StopReason::ToolUse);
        let calls = resp.tool_calls();
        assert_eq!(calls.len(), 1);
        if let ContentBlock::ToolUse { name, input, .. } = calls[0] {
            assert_eq!(name, "read_file");
            assert_eq!(input["path"], "/app/src/main.rs");
        }
    }

    #[test]
    fn custom_base_url() {
        let mut config = test_config();
        config.base_url = "https://openrouter.ai/api".into();
        let provider = OpenAiProvider::new(&config).unwrap();
        assert_eq!(provider.base_url, "https://openrouter.ai/api");
    }

    #[test]
    fn empty_api_key_returns_error() {
        let mut config = test_config();
        config.api_key = String::new();
        assert!(OpenAiProvider::new(&config).is_err());
    }
}
