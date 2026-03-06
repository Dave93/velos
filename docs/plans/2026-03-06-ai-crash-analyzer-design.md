# AI Crash Analyzer & Auto-Fixer — Design Document

**Date:** 2026-03-06
**Status:** Draft

## Overview

Встроенный в Velos AI-агент, который при краше процесса:
1. Анализирует причину ошибки (логи + исходный код)
2. Отправляет детальный анализ в Telegram с inline-кнопками "Fix" / "Ignore"
3. При нажатии "Fix" — запускает полноценный coding-агент с инструментами (read/edit/grep/run)
4. Отправляет результат фикса обратно в Telegram

Поддержка двух AI-провайдеров: Anthropic (Claude) + OpenAI-compatible (OpenAI, OpenRouter, Groq, Ollama, xAI и т.д.)

## Architecture

```
                         ~/.velos/config.toml
                                |
                        [ai] provider, model,
                        api_key, base_url
                        [notifications] language
                        [notifications.telegram]
                                |
    +-----------+          +----v-----+         +-------------+
    | Zig Daemon|--fork--->| notify-  |--HTTP-->| Telegram API|
    | (kqueue)  |  exec    | crash    |         | sendMessage |
    +-----+-----+         +----+-----+         | + inline KB |
          |                     |               +------+------+
          |  SIGCHLD            | 1. load config       |
          |  reaps              | 2. fetch logs (IPC)  | callback
          |                     | 3. gather code ctx   |
          |                     | 4. AI analysis call  |
          |                     | 5. send to Telegram  |
          |                     | 6. save crash record |
          |                     v                      v
          |              ~/.velos/crashes/     +-------+--------+
          |              <id>.json             | Telegram Poller|
          |                                    | (daemon thread)|
          |                                    +-------+--------+
          |                                            |
          |                                    fork+exec on "Fix"
          |                                            |
          |                                    +-------v--------+
          +--SIGCHLD reaps--<------------------| velos ai fix   |
                                               | <crash-id>     |
                                               +----------------+
                                               | Full agent loop|
                                               | with tools:    |
                                               | - read_file    |
                                               | - edit_file    |
                                               | - create_file  |
                                               | - delete_file  |
                                               | - grep         |
                                               | - glob         |
                                               | - list_dir     |
                                               | - run_command  |
                                               | - git_diff     |
                                               +----------------+
                                               | Result -> TG   |
                                               +----------------+
```

## Config Format

```toml
# ~/.velos/config.toml

[ai]
provider = "anthropic"              # "anthropic" | "openai"
model = "claude-sonnet-4-5"         # model ID
api_key = "sk-ant-..."              # API key
base_url = ""                       # custom endpoint (for openai-compatible)
max_iterations = 30                 # max agent loop iterations
auto_analyze = true                 # auto-analyze on crash (can disable)
auto_fix = false                    # auto-fix without asking (dangerous, off by default)

[notifications]
language = "en"                     # "en", "ru", "es", "zh", "ja", "de", "fr", etc.

[notifications.telegram]
bot_token = "123456:ABC..."
chat_id = "-100123456789"
```

## Provider Abstraction

### Unified Trait

```rust
// velos-ai/src/provider.rs

#[async_trait]
pub trait AiProvider: Send + Sync {
    /// Simple completion (analysis, no tools)
    async fn chat(&self, messages: &[Message], system: &str) -> Result<String>;

    /// Completion with tool use (agent loop)
    async fn chat_with_tools(
        &self,
        messages: &[Message],
        system: &str,
        tools: &[ToolDefinition],
    ) -> Result<AssistantResponse>;
}
```

### Anthropic Messages API

```
POST https://api.anthropic.com/v1/messages
Headers: x-api-key, anthropic-version: 2023-06-01
Body: { model, system, messages, tools?, max_tokens }

Tool use format:
  Assistant: { "type": "tool_use", "id": "...", "name": "...", "input": {...} }
  User:      { "type": "tool_result", "tool_use_id": "...", "content": "..." }
```

### OpenAI Chat Completions API

```
POST https://api.openai.com/v1/chat/completions  (or custom base_url)
Headers: Authorization: Bearer <key>
Body: { model, messages (system as role:system), tools?, max_tokens }

Tool use format:
  Assistant: { "tool_calls": [{ "id", "function": { "name", "arguments": "JSON" } }] }
  Tool:      { "role": "tool", "tool_call_id": "...", "content": "..." }
```

### Key Differences Handled by Abstraction

| Feature | Anthropic | OpenAI |
|---------|-----------|--------|
| System prompt | top-level `system` field | `role: "system"` message |
| Tool input | parsed JSON object | stringified JSON in `arguments` |
| Tool result | `role: "user"` + `tool_result` content block | `role: "tool"` message |
| Streaming | `event: content_block_delta` | `data: {"choices":[{"delta":...}]}` |
| Stop reason | `stop_reason: "tool_use"` | `finish_reason: "tool_calls"` |

## Tool System

### Built-in Tools

| Tool | Description | Parameters |
|------|-------------|------------|
| `read_file` | Read file contents | `path: string`, `offset?: int`, `limit?: int` |
| `edit_file` | Replace text in file | `path: string`, `old_text: string`, `new_text: string` |
| `create_file` | Create new file | `path: string`, `content: string` |
| `delete_file` | Delete a file | `path: string` |
| `grep` | Search file contents | `pattern: string`, `path?: string`, `glob?: string` |
| `glob` | Find files by pattern | `pattern: string`, `path?: string` |
| `list_dir` | List directory contents | `path: string` |
| `run_command` | Execute shell command | `command: string`, `cwd?: string`, `timeout_ms?: int` |
| `git_diff` | Show uncommitted changes | `path?: string` |

### Safety Constraints

- `run_command`: timeout 60s, no `rm -rf /`, no `sudo`, blocklist of dangerous commands
- `edit_file` / `delete_file`: only within process cwd (no escaping project dir)
- `create_file`: only within process cwd
- Max file read size: 100KB per call
- Max total tool calls per session: configurable `max_iterations` (default 30)

### Tool Definition Schema (shared, converted per provider)

```rust
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,  // JSON Schema
}
```

## Agent Loop

```
                    +------------------+
                    | System prompt +  |
                    | crash context    |
                    +--------+---------+
                             |
                    +--------v---------+
               +--->| Send to AI API   |
               |    | (with tools)     |
               |    +--------+---------+
               |             |
               |    +--------v---------+
               |    | Parse response   |
               |    +--------+---------+
               |             |
               |      +------+------+
               |      |             |
               |   tool_use      text_only
               |      |             |
               |  +---v---+    +----v----+
               |  |Execute|    | Done!   |
               |  | tools |    | Return  |
               |  +---+---+    +---------+
               |      |
               |  +---v----------+
               +--| Append result|
                  | to messages  |
                  +--------------+
```

**Max iterations**: configurable (default 30), prevents infinite loops.
**Token tracking**: count input/output tokens per session, log total cost.
**Error handling**: tool execution errors are returned to AI as tool_result with is_error=true.

## Crash Record Format

```json
// ~/.velos/crashes/<uuid>.json
{
  "id": "a1b2c3d4",
  "process_name": "runner-api",
  "exit_code": 1,
  "timestamp": "2026-03-06T14:30:00Z",
  "hostname": "macbook",
  "cwd": "/Users/macbookpro/development/runner",
  "logs": ["[ERR] TypeError: Cannot read property...", "..."],
  "analysis": "The crash is caused by...",
  "status": "pending",       // pending | fixing | fixed | ignored | failed
  "fix_result": null,        // filled after fix attempt
  "language": "en"
}
```

## Telegram Integration

### Analysis Message (with inline keyboard)

```
[Telegram Message]

🚨 Process Crashed

Name: `runner-api`
Exit code: `1`
Host: `macbook`
Time: 2026-03-06 14:30:00 UTC

📋 Last logs:
```
[ERR] TypeError: Cannot read property 'id' of undefined
    at handler (/app/src/routes/api.ts:42:15)
    at processTicksAndRejections (node:internal/process/task_queues:95:5)
```

🤖 AI Analysis:
The crash is caused by an unhandled null reference in `src/routes/api.ts` line 42.
The `request.params.id` is undefined when the route is called without an ID parameter.
The route handler doesn't validate that the `id` parameter exists before accessing it.

Suggested fix: Add a null check for `request.params.id` before line 42,
returning a 400 response if missing.

[Fix ✅]  [Ignore ❌]
```

### Callback Handling

- Inline keyboard with `callback_data`: `fix:<crash-id>` / `ignore:<crash-id>`
- Telegram Poller runs as daemon background thread
- Polls `getUpdates` with offset tracking
- On "Fix": fork+exec `velos ai fix <crash-id>`
- On "Ignore": update crash record status, send ack
- Answer callback query to remove "loading" state on button

### Localization

Messages localized via `notifications.language` config. Supported languages:
- `en` — English (default)
- `ru` — Russian

Template strings stored in `velos-ai/src/i18n.rs` as static maps.

Example keys:
- `crash.title` — "Process Crashed" / "Процесс упал"
- `crash.analysis_header` — "AI Analysis" / "AI-анализ"
- `crash.btn_fix` — "Fix" / "Исправить"
- `crash.btn_ignore` — "Ignore" / "Игнорировать"
- `fix.started` — "Starting fix..." / "Начинаю исправление..."
- `fix.completed` — "Fix applied!" / "Исправление применено!"
- `fix.failed` — "Fix failed" / "Не удалось исправить"

---

## Implementation Phases

### Phase 1: `velos-ai` Crate + Provider Abstraction

Create the new crate with provider trait and both implementations.

**Files:**
- `crates/velos-ai/Cargo.toml`
- `crates/velos-ai/src/lib.rs` — public API
- `crates/velos-ai/src/provider.rs` — `AiProvider` trait + `Message`, `AssistantResponse` types
- `crates/velos-ai/src/anthropic.rs` — Anthropic Messages API client
- `crates/velos-ai/src/openai.rs` — OpenAI Chat Completions API client
- `crates/velos-ai/src/types.rs` — shared types (Message, ToolCall, ToolResult, etc.)

**Tasks:**
- [ ] 1.1 Create crate skeleton with Cargo.toml (deps: ureq, serde, serde_json)
- [ ] 1.2 Define core types: Message (role + content blocks), ToolCall, ToolResult, AssistantResponse
- [ ] 1.3 Define AiProvider trait with `chat()` and `chat_with_tools()` methods
- [ ] 1.4 Implement AnthropicProvider (Messages API, tool_use format)
- [ ] 1.5 Implement OpenAiProvider (Chat Completions API, function calling format)
- [ ] 1.6 Factory function `create_provider(config) -> Box<dyn AiProvider>`
- [ ] 1.7 Unit tests: message serialization, tool call parsing for both providers

### Phase 2: Tool System

Implement the tool framework and all built-in tools.

**Files:**
- `crates/velos-ai/src/tools/mod.rs` — ToolDefinition, ToolRegistry, ToolExecutor trait
- `crates/velos-ai/src/tools/read_file.rs`
- `crates/velos-ai/src/tools/edit_file.rs`
- `crates/velos-ai/src/tools/create_file.rs`
- `crates/velos-ai/src/tools/delete_file.rs`
- `crates/velos-ai/src/tools/grep.rs`
- `crates/velos-ai/src/tools/glob.rs`
- `crates/velos-ai/src/tools/list_dir.rs`
- `crates/velos-ai/src/tools/run_command.rs`
- `crates/velos-ai/src/tools/git_diff.rs`

**Tasks:**
- [ ] 2.1 Define ToolDefinition struct and ToolExecutor trait
- [ ] 2.2 Implement ToolRegistry (register tools, get JSON schemas, dispatch by name)
- [ ] 2.3 Implement `read_file` tool (with offset/limit, max 100KB)
- [ ] 2.4 Implement `edit_file` tool (string replacement, path validation)
- [ ] 2.5 Implement `create_file` tool (with parent dir creation)
- [ ] 2.6 Implement `delete_file` tool (with path safety check)
- [ ] 2.7 Implement `grep` tool (regex search with ripgrep-like output)
- [ ] 2.8 Implement `glob` tool (file pattern matching)
- [ ] 2.9 Implement `list_dir` tool (recursive option, depth limit)
- [ ] 2.10 Implement `run_command` tool (timeout, blocklist, cwd sandboxing)
- [ ] 2.11 Implement `git_diff` tool
- [ ] 2.12 Safety layer: path sandboxing (restrict to project cwd), command blocklist
- [ ] 2.13 Unit tests for each tool

### Phase 3: Agent Loop

The core agent execution engine.

**Files:**
- `crates/velos-ai/src/agent.rs` — Agent struct, run loop, message history management

**Tasks:**
- [ ] 3.1 Define Agent struct (provider, tools, messages, config)
- [ ] 3.2 Implement agent loop: send → parse → execute tools → append → repeat
- [ ] 3.3 Handle stop conditions: text-only response, max iterations, error
- [ ] 3.4 Token/cost tracking (input_tokens, output_tokens per turn)
- [ ] 3.5 Error handling: tool errors returned as tool_result with is_error flag
- [ ] 3.6 Context management: truncate old messages if approaching context limit
- [ ] 3.7 Logging: log each turn (tool calls, results) to stderr for debugging
- [ ] 3.8 Integration test: mock provider + real tools on temp directory

### Phase 4: Config Extensions

Extend `~/.velos/config.toml` and `velos config` CLI.

**Files:**
- `crates/velos-cli/src/commands/config.rs` — extend GlobalConfig, add AI + language keys
- `crates/velos-cli/src/main.rs` — no new subcommands, just more config keys

**Tasks:**
- [ ] 4.1 Add `AiConfig` struct to GlobalConfig (provider, model, api_key, base_url, max_iterations, auto_analyze, auto_fix)
- [ ] 4.2 Add `language` field to NotificationsConfig
- [ ] 4.3 Extend `velos config set/get` for all new keys: `ai.provider`, `ai.model`, `ai.api_key`, `ai.base_url`, `ai.max_iterations`, `ai.auto_analyze`, `ai.auto_fix`, `notifications.language`
- [ ] 4.4 Mask `ai.api_key` in display (same as bot_token)
- [ ] 4.5 Validation: provider must be "anthropic" or "openai", max_iterations > 0

### Phase 5: Localization System

Multi-language support for all user-facing notification text.

**Files:**
- `crates/velos-ai/src/i18n.rs` — static message maps per language

**Tasks:**
- [ ] 5.1 Define `I18n` struct with `get(key) -> &str` method
- [ ] 5.2 Implement English (`en`) message set — all crash/fix/analysis strings
- [ ] 5.3 Implement Russian (`ru`) message set
- [ ] 5.4 Factory: `I18n::new(language_code)` with fallback to `en`
- [ ] 5.5 Define all message keys (crash.title, crash.analysis_header, crash.btn_fix, crash.btn_ignore, fix.started, fix.completed, fix.failed, fix.changes_summary, etc.)

### Phase 6: Enhanced Crash Analysis

Replace simple Telegram notification with AI-powered analysis.

**Files:**
- `crates/velos-cli/src/commands/notify_crash.rs` — major rewrite
- `crates/velos-ai/src/analyzer.rs` — crash context gathering + analysis prompt

**Tasks:**
- [ ] 6.1 Implement crash context gathering: logs + stack trace parsing + relevant source file detection
- [ ] 6.2 Build analysis prompt: system prompt with role + crash data + source code snippets
- [ ] 6.3 Call AI provider `chat()` (no tools) for analysis
- [ ] 6.4 Implement crash record persistence: write `~/.velos/crashes/<id>.json`
- [ ] 6.5 Update `notify_crash::run()`: load AI config, analyze, save record
- [ ] 6.6 Fallback: if AI not configured, send plain notification (current behavior)
- [ ] 6.7 Source file detection heuristic: parse stack traces for file paths (Node.js, Python, Rust, Go patterns)

### Phase 7: Telegram Inline Keyboard + Callback Polling

Add interactive buttons and callback handling.

**Files:**
- `crates/velos-ai/src/telegram.rs` — Telegram API helpers (sendMessage with inline KB, getUpdates, answerCallbackQuery)
- `crates/velos-ai/src/poller.rs` — Telegram callback poller (long-poll loop)

**Tasks:**
- [ ] 7.1 Extend `sendMessage` to support `reply_markup` with `InlineKeyboardMarkup`
- [ ] 7.2 Implement `getUpdates` with offset tracking for callback queries
- [ ] 7.3 Implement `answerCallbackQuery` to dismiss button loading state
- [ ] 7.4 Implement `editMessageText` to update message after action (show "Fixing..." / "Ignored")
- [ ] 7.5 Build Telegram poller loop: poll → parse callback_data → dispatch action
- [ ] 7.6 Callback data format: `fix:<crash-id>` / `ignore:<crash-id>`
- [ ] 7.7 Localized button text and status updates

### Phase 8: AI Fix Command

The full coding agent that fixes crashes.

**Files:**
- `crates/velos-cli/src/commands/ai.rs` — `velos ai fix`, `velos ai analyze`
- `crates/velos-cli/src/main.rs` — add `Ai` subcommand group

**Tasks:**
- [ ] 8.1 Add `Ai` subcommand group to CLI: `velos ai fix <crash-id>`, `velos ai analyze <process>`
- [ ] 8.2 Implement `velos ai fix <crash-id>`: load crash record, create Agent with tools, run loop
- [ ] 8.3 Build fix system prompt: role as senior developer, crash context, project structure, instructions
- [ ] 8.4 After fix: run validation (build/test if available), report result
- [ ] 8.5 Send fix result to Telegram: diff summary, files changed, test results
- [ ] 8.6 Git integration: optionally create branch + commit with descriptive message
- [ ] 8.7 Implement `velos ai analyze <process>`: manual trigger, fetch current logs, analyze without fixing
- [ ] 8.8 Error handling: if fix fails, report to Telegram with details, set crash status to "failed"

### Phase 9: Daemon Integration

Wire the Telegram poller into the daemon lifecycle.

**Files:**
- `crates/velos-cli/src/commands/daemon.rs` — spawn poller thread
- `crates/velos-ai/src/poller.rs` — expose `run_poller()` entry point

**Tasks:**
- [ ] 9.1 In `daemon.rs`: before `daemon_run()`, spawn background OS thread for Telegram poller
- [ ] 9.2 Poller thread creates its own tokio runtime, runs poll loop
- [ ] 9.3 On "Fix" callback: poller fork+execs `velos ai fix <crash-id>` (non-blocking)
- [ ] 9.4 On "Ignore" callback: update crash record, send ack to Telegram
- [ ] 9.5 Graceful shutdown: poller thread exits when daemon stops (check file/flag)
- [ ] 9.6 Only start poller if both Telegram and AI are configured
- [ ] 9.7 Crash record cleanup: auto-delete records older than 7 days

### Phase 10: Testing & Documentation

- [ ] 10.1 Integration test: mock AI server + real tools on temp project
- [ ] 10.2 Integration test: crash → analyze → fix flow end-to-end
- [ ] 10.3 Add `velos ai` to `--help` and README
- [ ] 10.4 Add config examples to docs
- [ ] 10.5 Test with real Anthropic API key on a sample crash
- [ ] 10.6 Test with OpenAI-compatible endpoint (OpenRouter)

---

## Dependencies (new)

| Crate | Version | Purpose |
|-------|---------|---------|
| `ureq` | 3 + json | HTTP client (already in velos-cli) |
| `serde` | 1 | Serialization |
| `serde_json` | 1 | JSON handling |
| `uuid` | 1 | Crash record IDs |
| `regex` | 1 | Stack trace parsing (already in velos-cli) |
| `walkdir` | 2 | Directory traversal for glob tool |

No heavy frameworks. All HTTP via `ureq` (sync, lightweight). Agent loop is synchronous — no need for async since it's a short-lived fork+exec'd process.

## Risk Mitigation

- **Runaway agent**: `max_iterations` cap (default 30) prevents infinite loops
- **Dangerous commands**: blocklist (`rm -rf`, `sudo`, `shutdown`, etc.) + cwd sandboxing
- **Cost control**: token tracking per session, logged to crash record
- **Auto-fix safety**: `auto_fix = false` by default — requires explicit Telegram approval
- **Multiple crashes**: each crash gets unique ID, poller dispatches independently
- **API failures**: graceful fallback to plain notification if AI provider unreachable
