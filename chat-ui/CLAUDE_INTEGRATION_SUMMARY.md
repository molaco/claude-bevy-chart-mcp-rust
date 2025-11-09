# Claude Chat SDK Integration - Executive Summary

## Quick Reference for chat-ui Implementation

Based on comprehensive analysis of the `japanese` codebase's `claude-agent-sdk-rust` implementation.

---

## Core Architecture

### Two Main APIs

1. **Simple Query API** (`query()`)
   - For stateless, one-shot interactions
   - Returns a stream of messages
   - No connection management needed
   - Best for: Simple prompts, CLI tools, batch processing

2. **Interactive Client API** (`ClaudeSDKClient`)
   - For stateful, bidirectional conversations
   - Send messages while receiving responses
   - Lock-free concurrent operations
   - Best for: Chat UI, REPL-like interfaces, long sessions

### Key Data Types

**Messages**:
- `Message::User` - User input
- `Message::Assistant` - Claude's response
- `Message::Result` - Session end with metrics
- `Message::System` - System notifications
- `Message::StreamEvent` - Partial updates

**Content Blocks**:
- `ContentBlock::Text` - Plain text
- `ContentBlock::Thinking` - Extended thinking (Claude 3.7+)
- `ContentBlock::ToolUse` - Tool invocation request
- `ContentBlock::ToolResult` - Tool execution result

---

## Communication Flow

```
┌────────────────────────────────────────────────────────┐
│           ClaudeSDKClient (Main Thread)                │
├────────────────────────────────────────────────────────┤
│                                                        │
│  send_message("Hello")                                │
│          ↓                                             │
│  control_tx → [Control Writer Task] ────┐            │
│                                          ↓            │
│                                   [SubprocessTransport]
│                                   (Arc<Mutex<>>)      │
│                                          ↓            │
│                                   [Claude CLI Process]│
│                                          ↓            │
│                                   [SubprocessTransport]
│                                          ↓            │
│              ┌──────────────────────────┘            │
│              ↓                                        │
│  [Message Reader Task] → message_rx                  │
│              ↓                                        │
│  next_message() ←────────────────────────────────────│
│
└────────────────────────────────────────────────────────┘
```

### Background Tasks (Spawned Automatically)

1. **Message Reader** - Reads CLI output, parses JSON, dispatches to appropriate handler
2. **Control Writer** - Writes user messages and control commands to CLI
3. **Hook Handler** - Processes hook events (if configured)
4. **Hook Callback Handler** - Processes hook_callback requests from CLI
5. **Permission Handler** - Processes permission requests (if configured)
6. **MCP Message Handler** - Processes MCP requests from SDK servers (if configured)

No manual task management needed - all handled internally.

---

## Transport Layer

### CLI Command Execution

The SDK wraps the Claude Code CLI:

```bash
claude [mode-flags] [config-flags]
```

**Streaming Mode** (for interactive client):
```bash
claude --input-format stream-json --output-format stream-json --verbose
```

**Print Mode** (for one-shot queries):
```bash
claude --print --output-format stream-json --verbose
```

### Message Format

- **Input**: JSON objects, one per line on stdin
- **Output**: JSON objects, one per line on stdout
- **Session**: Persistence via `--resume <session-id>` flag

### Security Features

- Environment variable filtering (blocks LD_PRELOAD, PATH, NODE_OPTIONS, etc.)
- CLI argument allowlist (timeout, retries, log-level, cache-dir only)
- 1MB default max buffer size (prevents DoS from malformed messages)
- Bounds checking on configurable values (max_turns <= 1000)

---

## Message Parsing and Handling

### Simple Parsing

```rust
pub fn parse_message(data: serde_json::Value) -> Result<Message> {
    serde_json::from_value(data).map_err(|e| 
        ClaudeError::message_parse(format!("Failed: {e}"), Some(data))
    )
}
```

### Type Discrimination

Control messages are intercepted before regular parsing:
- `control_response` - CLI response to our init/control messages (filtered out)
- `control_request` - CLI callback requests (routed to handlers)
  - `hook_callback` - Hooks that need processing
  - `mcp_message` - MCP tool invocations

Regular messages that reach your code:
- `user` - User input
- `assistant` - Claude's response
- `result` - Session metrics
- `system` - Notifications
- `stream_event` - Partial updates

---

## Async/Concurrency Model

### Synchronization Primitives

**Mutexes** (Arc<Mutex<T>>):
- SubprocessTransport - CLI process I/O
- ProtocolHandler - Message serialization
- HookManager - Hook registry
- PermissionManager - Permission checking

**Channels** (mpsc::UnboundedChannel):
- message_rx - Parsed messages to app
- control_tx - Outgoing commands
- hook_rx/hook_callback_rx - Hook events
- permission_rx - Permission requests
- mcp_callback_rx - MCP requests

### Lock-Free Pattern

Critical insight: **Reader never holds lock**

1. Reader task acquires transport lock **once** to get receiver channel
2. Reader releases lock immediately
3. Reader processes messages from channel (no lock needed)
4. Writer task locks transport briefly for each write
5. Result: **No contention** between read and write

This allows safe concurrent send/receive without blocking.

---

## Configuration (ClaudeAgentOptions)

### Builder Pattern

```rust
let options = ClaudeAgentOptions::builder()
    .system_prompt("You are a helpful assistant")
    .max_turns(10)
    .add_allowed_tool("Read")
    .add_allowed_tool("Bash")
    .permission_mode(PermissionMode::Default)
    .build();
```

### Common Options

| Option | Type | Usage |
|--------|------|-------|
| `system_prompt` | String/Preset | Control Claude's behavior |
| `max_turns` | u32 | Limit conversation length |
| `allowed_tools` | Vec<ToolName> | Restrict available tools |
| `disallowed_tools` | Vec<ToolName> | Block specific tools |
| `permission_mode` | PermissionMode | Control permission prompts |
| `can_use_tool` | Callback | Custom permission logic |
| `hooks` | HashMap<HookEvent, Vec> | Intercept events |
| `mcp_servers` | HashMap | Custom tools (SDK MCP) |
| `model` | String | Specify model (e.g., "opus") |
| `resume` | SessionId | Continue previous session |
| `cwd` | PathBuf | Working directory |
| `settings` | PathBuf | Settings file path |

---

## Hooks System

### Hook Events

```rust
pub enum HookEvent {
    PreToolUse,      // Before tool executes
    PostToolUse,     // After tool executes
    UserPromptSubmit,// When user submits
    Stop,            // When conversation stops
    SubagentStop,    // When subagent stops
    PreCompact,      // Before compacting
}
```

### Hook Matching

```rust
// Match specific tool
HookMatcher {
    matcher: Some("Bash".to_string()),
    hooks: vec![callback],
}

// Match multiple tools
HookMatcher {
    matcher: Some("Write|Edit".to_string()),
    hooks: vec![callback],
}

// Match all tools
HookMatcher {
    matcher: None,
    hooks: vec![callback],
}
```

### Hook Callback

```rust
pub type HookCallback = Arc<
    dyn Fn(
        serde_json::Value,        // Event data
        Option<String>,           // Tool name
        HookContext,
    ) -> Pin<Box<dyn Future<Output = Result<HookOutput>> + Send>>
        + Send + Sync,
>;
```

### Hook Output

```rust
pub struct HookOutput {
    pub decision: Option<HookDecision>,      // Block?
    pub system_message: Option<String>,      // Add to system
    pub hook_specific_output: Option<Value>, // Custom data
}
```

---

## Permissions System

### Permission Check Flow

1. Check `disallowed_tools` (deny if found)
2. Check `allowed_tools` if set (deny if not listed)
3. Invoke custom callback if provided
4. Default to allow

### Permission Callback

```rust
pub type CanUseToolCallback = Arc<
    dyn Fn(
        ToolName,
        serde_json::Value,           // Tool input
        ToolPermissionContext,
    ) -> Pin<Box<dyn Future<Output = Result<PermissionResult>> + Send>>
        + Send + Sync,
>;
```

### Permission Result

```rust
pub enum PermissionResult {
    Allow(PermissionResultAllow),
    Deny(PermissionResultDeny),
}

pub struct PermissionResultAllow {
    pub updated_input: Option<Value>,              // Modify input
    pub updated_permissions: Option<Vec<PermissionUpdate>>,
}

pub struct PermissionResultDeny {
    pub message: String,
    pub interrupt: bool,  // Stop conversation?
}
```

---

## MCP (Model Context Protocol) Integration

### Two MCP Types

**1. External MCP Servers** (subprocess-based):
- Stdio: Direct child process
- SSE: Server-Sent Events over HTTP
- HTTP: JSON-RPC over HTTP

**2. SDK MCP Servers** (in-process):
- No subprocess overhead
- Direct function calls
- Full stack traces
- Type-safe Rust implementations

### SDK MCP Server Definition

```rust
let tool = SdkMcpTool::new(
    "calculator",
    "A calculator tool",
    json!({
        "type": "object",
        "properties": {
            "operation": {"type": "string"},
            "a": {"type": "number"},
            "b": {"type": "number"},
        }
    }),
    |input| Box::pin(async move {
        let operation = input["operation"].as_str().unwrap_or("add");
        let a = input["a"].as_f64().unwrap_or(0.0);
        let b = input["b"].as_f64().unwrap_or(0.0);
        
        let result = match operation {
            "add" => a + b,
            "sub" => a - b,
            _ => 0.0,
        };
        
        Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: format!("Result: {}", result),
            }],
            is_error: None,
        })
    }),
);

let server = SdkMcpServer::new("calc")
    .version("1.0.0")
    .tool(tool);
```

### JSONRPC Protocol

Follows JSON-RPC 2.0:

```json
{
  "jsonrpc": "2.0",
  "method": "tools/list" | "tools/call",
  "params": {...},
  "id": "request-id"
}
```

---

## Error Handling

### Error Types

```rust
pub enum ClaudeError {
    CliNotFound(String),              // Claude Code not installed
    Connection(String),               // Connection failed
    Process { message, exit_code, stderr },
    JsonDecode(serde_json::Error),    // JSON parsing error
    MessageParse { message, data },   // Message deserialization
    Transport(String),                // Transport layer error
    ControlProtocol(String),          // Protocol error
    Hook(String),                     // Hook execution error
    Mcp(String),                      // MCP error
    ValidationError(String),          // Schema validation
    Io(std::io::Error),               // IO error
    Timeout(String),                  // Operation timeout
    InvalidConfig(String),            // Invalid configuration
}
```

### Error Context

Errors include:
- Exit codes and stderr for process errors
- Raw JSON data for parse errors
- Helpful messages for missing CLI

```rust
match error {
    ClaudeError::CliNotFound(msg) => {
        eprintln!("Install with: npm install -g @anthropic-ai/claude-code");
    }
    ClaudeError::MessageParse { message, data } => {
        eprintln!("Failed to parse: {} (raw: {})", message, data);
    }
    _ => eprintln!("Error: {}", error),
}
```

---

## Important Gotchas

1. **Control messages don't appear in stream**
   - control_response and control_request are filtered out
   - Only User/Assistant/Result/System/StreamEvent reach your code

2. **Hook callback IDs must match**
   - Generated as `hook_0`, `hook_1`, etc.
   - Used to route CLI callbacks back to correct Rust closure

3. **Streaming mode activation**
   - Automatically enabled when using `ClaudeSDKClient`
   - Automatically enabled when hooks/permissions/SDK MCP configured
   - Otherwise uses print mode for efficiency

4. **Permission defaults**
   - No allowed_tools list = all tools allowed
   - With allowed_tools list = only those allowed
   - Disallowed_tools checked first (deny wins)

5. **Interrupt may not work**
   - SDK sends interrupt correctly
   - CLI may not process control messages in all versions
   - Architecture supports it for future compatibility

6. **Reader acquires lock once**
   - Receiver is obtained and released immediately
   - No holding lock during message processing
   - Safe for concurrent reads/writes

7. **Channel closures are automatic**
   - Background tasks exit when channels drop
   - No manual task cancellation needed
   - Resources cleaned up via Arc drop handlers

---

## Integration Checklist for chat-ui

- [ ] Choose API: `query()` for simple, `ClaudeSDKClient` for interactive
- [ ] Create `ClaudeAgentOptions` with builder
- [ ] Configure system prompt if needed
- [ ] Set allowed/disallowed tools
- [ ] Add hooks if event interception needed
- [ ] Add permissions callback if custom control needed
- [ ] Add SDK MCP servers if custom tools needed
- [ ] Spawn client/query
- [ ] Iterate through messages with `next_message()` or stream
- [ ] Handle different message types (User, Assistant, Result)
- [ ] Extract text from `ContentBlock::Text`
- [ ] Call `close()` when done
- [ ] Handle errors with context from `ClaudeError`

---

## File Reference

See `CLAUDE_SDK_REFERENCE.md` for complete implementation details including:
- Full source code patterns
- Complete background task implementations
- Full control protocol specification
- Complete permission/hook APIs
- Full MCP server implementation
- Example usage patterns
- Security implementation details

---

## Summary

The claude-agent-sdk-rust is a well-architected, production-ready SDK that provides:

1. **Dual APIs** - Simple queries or interactive client
2. **Stream-based communication** - All messages flow through channels
3. **Automatic concurrency** - Multiple background tasks managed internally
4. **Lock-free reads/writes** - No contention between send and receive
5. **Extensible architecture** - Hooks, permissions, and custom MCP tools
6. **Comprehensive error handling** - Rich context for debugging
7. **Security-hardened** - Environment and argument validation
8. **Type-safe configuration** - Builder pattern with sensible defaults

For chat-ui, the recommended approach is:
1. Use `ClaudeSDKClient` for interactive conversations
2. Use `query()` for one-shot operations
3. Leverage background tasks for concurrent message handling
4. Use hooks for monitoring tool usage
5. Use permissions for fine-grained control
6. Use SDK MCP for custom tools

