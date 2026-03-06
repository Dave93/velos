use std::path::PathBuf;

use crate::provider::{AiError, AiProvider};
use crate::tools::ToolRegistry;
use crate::types::*;

// ---------------------------------------------------------------------------
// Agent result
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct AgentResult {
    /// Final text response from the AI.
    pub final_text: String,
    /// Total tokens used across all iterations.
    pub total_usage: Usage,
    /// Number of AI request iterations.
    pub iterations: u32,
    /// Total number of tool calls executed.
    pub tool_calls: u32,
}

// ---------------------------------------------------------------------------
// Agent
// ---------------------------------------------------------------------------

pub struct Agent {
    provider: Box<dyn AiProvider>,
    tools: ToolRegistry,
    system: String,
    cwd: PathBuf,
    max_iterations: u32,
}

impl Agent {
    pub fn new(
        provider: Box<dyn AiProvider>,
        tools: ToolRegistry,
        system: String,
        cwd: PathBuf,
        max_iterations: u32,
    ) -> Self {
        Self {
            provider,
            tools,
            system,
            cwd,
            max_iterations,
        }
    }

    /// Run the agent loop with an initial user message.
    /// Returns the final result after the AI is done (or max iterations reached).
    pub fn run(&self, initial_message: &str) -> Result<AgentResult, AiError> {
        let mut messages = vec![Message::user(initial_message)];
        let tool_defs = self.tools.definitions();

        let mut total_usage = Usage::default();
        let mut iterations = 0u32;
        let mut tool_calls_total = 0u32;

        loop {
            iterations += 1;

            if iterations > self.max_iterations {
                eprintln!(
                    "[velos-ai] max iterations ({}) reached, stopping agent",
                    self.max_iterations
                );
                break;
            }

            eprintln!("[velos-ai] iteration {iterations}/{}", self.max_iterations);

            // Send to AI
            let response = self.provider.chat_with_tools(
                &messages,
                &self.system,
                &tool_defs,
            )?;

            // Track usage
            total_usage.input_tokens += response.usage.input_tokens;
            total_usage.output_tokens += response.usage.output_tokens;

            // Log any text output
            let text = response.text();
            if !text.is_empty() {
                eprintln!("[velos-ai] assistant: {}...", truncate_log(&text, 200));
            }

            // Check if we need to execute tools
            let tool_use_blocks: Vec<_> = response
                .content
                .iter()
                .filter(|b| matches!(b, ContentBlock::ToolUse { .. }))
                .collect();

            if tool_use_blocks.is_empty() || response.stop_reason != StopReason::ToolUse {
                // No tools — we're done
                eprintln!(
                    "[velos-ai] done: {iterations} iterations, {tool_calls_total} tool calls, \
                     {} in + {} out tokens",
                    total_usage.input_tokens, total_usage.output_tokens
                );
                return Ok(AgentResult {
                    final_text: text,
                    total_usage,
                    iterations,
                    tool_calls: tool_calls_total,
                });
            }

            // Append the assistant message (with tool_use blocks) to history
            messages.push(Message {
                role: Role::Assistant,
                content: response.content.clone(),
            });

            // Execute each tool and collect results
            let mut tool_results = Vec::new();
            for block in &tool_use_blocks {
                if let ContentBlock::ToolUse { id, name, input } = block {
                    eprintln!("[velos-ai] tool: {name}({})", truncate_log(&input.to_string(), 100));
                    tool_calls_total += 1;

                    let (content, is_error) = match self.tools.execute(name, input.clone(), &self.cwd) {
                        Ok(output) => {
                            eprintln!(
                                "[velos-ai]   -> ok ({} bytes)",
                                output.len()
                            );
                            (output, false)
                        }
                        Err(err) => {
                            eprintln!("[velos-ai]   -> error: {err}");
                            (err, true)
                        }
                    };

                    tool_results.push(ContentBlock::ToolResult {
                        tool_use_id: id.clone(),
                        content,
                        is_error,
                    });
                }
            }

            // Append tool results as a message
            // For Anthropic: role "user" with tool_result content blocks
            // For OpenAI: role "tool" with individual messages
            // The provider handles the conversion from our internal format
            messages.push(Message {
                role: Role::Tool,
                content: tool_results,
            });
        }

        // Reached max iterations — return whatever text we have from the last response
        // Try one final call without tools to get a summary
        eprintln!("[velos-ai] requesting final summary after max iterations");
        messages.push(Message::user(
            "You have reached the maximum number of iterations. Please provide your final summary \
             of what you found and any changes you made."
        ));

        let final_resp = self.provider.chat(&messages, &self.system)?;
        total_usage.input_tokens += 100; // approximate
        total_usage.output_tokens += 100;

        Ok(AgentResult {
            final_text: final_resp,
            total_usage,
            iterations,
            tool_calls: tool_calls_total,
        })
    }

    /// Simple analysis mode — no tools, single API call.
    pub fn analyze(&self, prompt: &str) -> Result<String, AiError> {
        let messages = vec![Message::user(prompt)];
        self.provider.chat(&messages, &self.system)
    }
}

fn truncate_log(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.replace('\n', " ")
    } else {
        format!("{}...", s[..max].replace('\n', " "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::default_registry;

    /// A mock provider that returns a fixed sequence of responses.
    struct MockProvider {
        responses: std::sync::Mutex<Vec<AssistantResponse>>,
    }

    impl MockProvider {
        fn new(responses: Vec<AssistantResponse>) -> Self {
            Self {
                responses: std::sync::Mutex::new(responses),
            }
        }
    }

    impl AiProvider for MockProvider {
        fn chat(&self, _messages: &[Message], _system: &str) -> Result<String, AiError> {
            let mut resps = self.responses.lock().unwrap();
            if resps.is_empty() {
                return Ok("done".into());
            }
            let resp = resps.remove(0);
            Ok(resp.text())
        }

        fn chat_with_tools(
            &self,
            _messages: &[Message],
            _system: &str,
            _tools: &[ToolDefinition],
        ) -> Result<AssistantResponse, AiError> {
            let mut resps = self.responses.lock().unwrap();
            if resps.is_empty() {
                return Ok(AssistantResponse {
                    content: vec![ContentBlock::Text { text: "done".into() }],
                    stop_reason: StopReason::EndTurn,
                    usage: Usage::default(),
                });
            }
            Ok(resps.remove(0))
        }
    }

    #[test]
    fn agent_simple_text_response() {
        let provider = MockProvider::new(vec![AssistantResponse {
            content: vec![ContentBlock::Text {
                text: "The bug is caused by a null pointer.".into(),
            }],
            stop_reason: StopReason::EndTurn,
            usage: Usage {
                input_tokens: 100,
                output_tokens: 20,
            },
        }]);

        let agent = Agent::new(
            Box::new(provider),
            default_registry(),
            "You are a debugger.".into(),
            std::env::temp_dir(),
            30,
        );

        let result = agent.run("Why did my app crash?").unwrap();
        assert_eq!(result.final_text, "The bug is caused by a null pointer.");
        assert_eq!(result.iterations, 1);
        assert_eq!(result.tool_calls, 0);
    }

    #[test]
    fn agent_tool_use_then_response() {
        let temp = std::env::temp_dir().join("velos_agent_test");
        let _ = std::fs::create_dir_all(&temp);
        std::fs::write(temp.join("test.txt"), "hello world\nline two\n").unwrap();

        let provider = MockProvider::new(vec![
            // First response: tool call
            AssistantResponse {
                content: vec![ContentBlock::ToolUse {
                    id: "tu_1".into(),
                    name: "read_file".into(),
                    input: serde_json::json!({"path": "test.txt"}),
                }],
                stop_reason: StopReason::ToolUse,
                usage: Usage { input_tokens: 50, output_tokens: 10 },
            },
            // Second response: final text
            AssistantResponse {
                content: vec![ContentBlock::Text {
                    text: "The file contains hello world.".into(),
                }],
                stop_reason: StopReason::EndTurn,
                usage: Usage { input_tokens: 80, output_tokens: 15 },
            },
        ]);

        let agent = Agent::new(
            Box::new(provider),
            default_registry(),
            "You are a debugger.".into(),
            temp.clone(),
            30,
        );

        let result = agent.run("Read test.txt").unwrap();
        assert_eq!(result.final_text, "The file contains hello world.");
        assert_eq!(result.iterations, 2);
        assert_eq!(result.tool_calls, 1);
        assert_eq!(result.total_usage.input_tokens, 130);

        let _ = std::fs::remove_dir_all(&temp);
    }

    #[test]
    fn agent_max_iterations() {
        // Provider always returns tool_use — agent should stop at max_iterations
        let responses: Vec<AssistantResponse> = (0..5)
            .map(|i| AssistantResponse {
                content: vec![ContentBlock::ToolUse {
                    id: format!("tu_{i}"),
                    name: "list_dir".into(),
                    input: serde_json::json!({}),
                }],
                stop_reason: StopReason::ToolUse,
                usage: Usage { input_tokens: 10, output_tokens: 5 },
            })
            .collect();

        let provider = MockProvider::new(responses);
        let agent = Agent::new(
            Box::new(provider),
            default_registry(),
            "test".into(),
            std::env::temp_dir(),
            3, // max 3 iterations
        );

        let result = agent.run("loop forever").unwrap();
        assert!(result.iterations <= 4); // 3 tool loops + 1 final summary
    }
}
