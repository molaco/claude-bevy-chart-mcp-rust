# Claude SDK Implementation - Key Files Reference

## Location
`/home/molaco/Documents/japanese/claude-agent-sdk-rust/`

---

## Core Implementation Files

### 1. Client API
**File**: `src/client/mod.rs` (1000+ lines)

**Key Types**:
- `ClaudeSDKClient` - Main interactive client
- `ControlRequest` enum - Outgoing commands
- 6 background task implementations

**Key Methods**:
- `new(options, cli_path)` - Create client
- `send_message(content)` - Send user message
- `next_message()` - Receive next message
- `interrupt()` - Send interrupt
- `close()` - Cleanup
- `respond_to_hook(hook_id, response)` - Hook responses
- `respond_to_permission(request_id, result)` - Permission responses

**Critical Pattern**: Lock-free concurrent read/write via task separation

---

### 2. Simple Query API
**File**: `src/query.rs` (100 lines)

**Key Function**:
```rust
pub async fn query(
    prompt: impl Into<String>,
    options: Option<ClaudeAgentOptions>,
) -> Result<impl Stream<Item = Result<Message>>>
```

**Use Case**: Stateless one-shot queries

---

### 3. Type Definitions
**File**: `src/types.rs` (1000+ lines)

**Newtypes**:
- `SessionId` - Type-safe session ID
- `ToolName` - Type-safe tool name
- `RequestId` - Type-safe request ID

**Key Enums**:
- `Message` - All message types
- `ContentBlock` - Message content (Text, ToolUse, etc.)
- `HookEvent` - Hook lifecycle events
- `PermissionResult` - Permission allow/deny
- `ClaudeAgentOptions` - Configuration builder

**Message Types**:
- User, Assistant, System, Result, StreamEvent

---

### 4. Transport Layer
**File**: `src/transport/subprocess.rs` (500+ lines)

**Key Structure**:
- `SubprocessTransport` - Wraps Claude Code CLI process
- `PromptInput` - String or Stream mode

**Key Methods**:
- `new(prompt, options, cli_path)` - Create transport
- `connect()` - Start subprocess
- `write(data)` - Write to stdin
- `read_messages()` - Get message receiver channel
- `close()` - Cleanup
- `find_cli()` - Locate Claude Code binary

**CLI Command Building**:
- Streaming mode: `--input-format stream-json --output-format stream-json`
- Print mode: `--print --output-format stream-json`
- Configuration flags: system-prompt, allowedTools, max-turns, etc.

**Security**:
- Dangerous env vars blocked
- CLI flags allowlisted
- Buffer size limits

---

### 5. Message Parsing
**File**: `src/message/parser.rs` (30 lines)

**Key Function**:
```rust
pub fn parse_message(data: serde_json::Value) -> Result<Message>
```

Simple serde-based deserialization with context-rich errors.

---

### 6. Control Protocol
**File**: `src/control/protocol.rs` (500+ lines)

**Key Types**:
- `ControlMessage` - Envelope for all protocol messages
- `ControlRequest` - SDK to CLI commands
- `ControlResponse` - CLI to SDK responses
- `InitRequest` / `InitResponse` - Handshake
- `ProtocolHandler` - Message serialization/deserialization

**Message Types**:
- Interrupt, SendMessage, HookResponse, PermissionResponse
- HookCallbackResponse, McpMessageResponse

**Request ID Management**:
```rust
static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(1);
// Generates: "req_<counter>_<nanos>"
```

---

### 7. Hooks System
**File**: `src/hooks/mod.rs` (200+ lines)

**Key Types**:
- `HookManager` - Registry and dispatcher
- `HookMatcher` - Pattern matcher for tools
- `HookCallback` - Async callback type
- `HookOutput` - Response data

**Key Methods**:
- `register_with_ids(matcher)` - Register and generate callback IDs
- `invoke_by_id(callback_id, event_data, tool_name, context)` - Invoke by ID
- `invoke(event_data, tool_name, context)` - Invoke by pattern

**Callback ID Mapping**:
```rust
callback_id_map: HashMap<String, (usize, usize)>
// Maps "hook_N" -> (matcher_index, callback_index)
```

---

### 8. Permissions System
**File**: `src/permissions/mod.rs` (150+ lines)

**Key Types**:
- `PermissionManager` - Check and enforce permissions
- `PermissionRequest` - Incoming request
- `PermissionResult` - Allow/Deny response
- `CanUseToolCallback` - Custom permission logic

**Key Methods**:
- `can_use_tool(tool_name, tool_input, context)` - Check permission
- `set_callback(callback)` - Set custom logic
- `set_allowed_tools(tools)` - Restrict to list
- `set_disallowed_tools(tools)` - Block specific tools

**Permission Check Order**:
1. Check disallowed_tools (deny if found)
2. Check allowed_tools if set (deny if not listed)
3. Invoke callback if set
4. Default to allow

---

### 9. MCP Integration
**File**: `src/mcp/` (4 files)

#### 9a. SDK MCP Server
**File**: `src/mcp/server.rs`

**Key Type**:
- `SdkMcpServer` - In-process MCP server

**Key Methods**:
- `new(name)` - Create server
- `version(version)` - Set version
- `tool(tool)` - Register tool
- `handle_request(request)` - Process JSONRPC request

#### 9b. MCP Tools
**File**: `src/mcp/tool.rs`

**Key Type**:
- `SdkMcpTool` - Individual tool
- `ToolResult` - Tool output
- `ToolContent` - Result content (Text, Image)

**Handler Signature**:
```rust
pub type ToolHandler = Arc<
    dyn Fn(Value) -> Pin<Box<dyn Future<Output = Result<ToolResult>> + Send>>
        + Send + Sync,
>;
```

#### 9c. JSONRPC Protocol
**File**: `src/mcp/protocol.rs`

**Key Types**:
- `JsonRpcRequest` - Standard JSONRPC request
- `JsonRpcResponse` - Standard JSONRPC response
- `McpError` - Error structure

**Methods**:
- `tools/list` - List available tools
- `tools/call` - Invoke a tool

---

### 10. Error Types
**File**: `src/error.rs` (150 lines)

**Key Types**:
- `ClaudeError` - Main error enum
- `Result<T>` - Standard result type

**Error Variants**:
- CliNotFound
- Connection
- Process (with exit_code and stderr)
- JsonDecode / MessageParse
- Transport / ControlProtocol
- Hook / Mcp
- ValidationError / Io / Timeout
- InvalidConfig

**Error Helpers**:
- `cli_not_found()` - With installation instructions
- `message_parse(msg, data)` - With raw data
- `process(msg, exit_code, stderr)` - With context

---

### 11. Library Root
**File**: `src/lib.rs` (300 lines)

**Public Re-exports**:
- `ClaudeSDKClient`
- `query()`
- `Message`, `ContentBlock`
- `ClaudeAgentOptions`, `ClaudeAgentOptionsBuilder`
- `HookManager`, `PermissionManager`
- `Transport`, `SubprocessTransport`

**Documentation**:
- Comprehensive module documentation
- Usage examples for each major API
- Feature flags (http, tracing-support)

---

## Example Files

### 1. Simple Query
**File**: `examples/simple_query.rs` (80 lines)

Shows:
- Creating options with builder
- Calling `query()`
- Iterating through message stream
- Extracting text from ContentBlock

### 2. Interactive Client
**File**: `examples/interactive_client.rs` (70 lines)

Shows:
- Creating `ClaudeSDKClient`
- Sending messages
- Receiving responses
- Proper cleanup with `close()`

### 3. Bidirectional Demo
**File**: `examples/bidirectional_demo.rs` (150 lines)

Shows:
- Concurrent read/write
- `tokio::select!` for concurrent operations
- Interrupt mechanism
- Multiple message exchanges

### 4. Hooks Demo
**File**: `examples/hooks_demo.rs` (200+ lines)

Shows:
- Creating hook callbacks
- Pattern matching on tools
- Hook output with decisions
- Registering with options

### 5. Permissions Demo
**File**: `examples/permissions_demo.rs` (200+ lines)

Shows:
- Permission callbacks
- Allow/Deny responses
- Tool-specific restrictions
- Integration with query()

### 6. MCP Demo
**File**: `examples/mcp_demo.rs` (300+ lines)

Shows:
- Creating SDK MCP tools
- Tool handler implementation
- MCP server registration
- Tool invocation via Claude

---

## Configuration Files

### Cargo.toml
**Key Dependencies**:
- tokio (async runtime)
- serde / serde_json (serialization)
- async-trait (trait async support)
- thiserror (error types)
- futures (streaming)
- which (CLI discovery)
- jsonschema (validation)

**Optional**:
- reqwest (HTTP MCP support)
- tracing (structured logging)

---

## Key Patterns and Insights

### 1. Message Flow
```
User Code
    ↓
send_message(msg)
    ↓
control_tx (unbounded channel)
    ↓
control_writer_task
    ↓
transport.write()
    ↓
CLI stdin
    ↓
Claude CLI
    ↓
CLI stdout
    ↓
message_reader_task (reads continuously)
    ↓
message_tx (unbounded channel)
    ↓
next_message() ← User Code
```

### 2. Background Task Startup
```rust
// 1. Create channels
let (message_tx, message_rx) = mpsc::unbounded_channel();
let (control_tx, control_rx) = mpsc::unbounded_channel();

// 2. Spawn tasks
tokio::spawn(message_reader_task(..., message_tx));
tokio::spawn(control_writer_task(..., control_rx));

// 3. Return handles
Self {
    message_rx,
    control_tx,
    ...
}
```

### 3. Mutex Pattern
```rust
// Acquire lock briefly
{
    let mut guard = mutex.lock().await;
    guard.do_something();
    drop(guard); // explicit drop to ensure release
} // guard dropped here

// Alternative: guard drops at scope end
let guard = mutex.lock().await;
// use guard
// drops at end of scope
```

### 4. Type-Safe Newtypes
```rust
pub struct SessionId(String);

impl SessionId {
    pub fn new(id: impl Into<String>) -> Self { Self(id.into()) }
    pub fn as_str(&self) -> &str { &self.0 }
}

// Prevents mixing with other strings at compile time
```

### 5. Builder Pattern
```rust
ClaudeAgentOptions::builder()
    .system_prompt("...")
    .max_turns(10)
    .add_allowed_tool("Read")
    .can_use_tool(callback)
    .build()
```

---

## Dependencies Mapping

```
claude-agent-sdk-rust/
├── tokio (async runtime)
│   └── Used in: transport, client, all async operations
├── serde + serde_json (serialization)
│   └── Used in: message parsing, CLI config, transport
├── async-trait (async trait methods)
│   └── Used in: Transport trait
├── thiserror (error types)
│   └── Used in: error.rs
├── futures (streaming)
│   └── Used in: query() return type
├── which (find binaries)
│   └── Used in: find_cli()
├── jsonschema (validation)
│   └── Used in: MCP tool schema validation
├── tokio-util (codec support)
│   └── Used in: line-based message framing
└── async-stream (stream macros)
    └── Used in: query() streaming
```

---

## Quick Code Locations

| Concept | File | Lines |
|---------|------|-------|
| Client creation | `client/mod.rs` | 221-415 |
| Message reading | `client/mod.rs` | 417-550 |
| Control writing | `client/mod.rs` | 552-619 |
| Hook handling | `client/mod.rs` | 621-728 |
| Permission handling | `client/mod.rs` | 844-886 |
| CLI execution | `transport/subprocess.rs` | 88-115 |
| CLI building | `transport/subprocess.rs` | 143-340 |
| Message reading | `transport/subprocess.rs` | ~400 |
| Hook registration | `hooks/mod.rs` | 48-66 |
| Hook invocation | `hooks/mod.rs` | 78-94 |
| Permission check | `permissions/mod.rs` | 57-95 |
| MCP server | `mcp/server.rs` | ~100 |
| MCP handling | `client/mod.rs` | 730-842 |

---

## Integration Points for chat-ui

1. **Use ClaudeSDKClient** - Interactive client for Bevy UI
2. **Options Builder** - Configure system prompt, tools, permissions
3. **Message Stream** - Iterate with `next_message()`
4. **Content Blocks** - Extract text from messages
5. **Error Handling** - Use rich error context
6. **Hooks (optional)** - Monitor tool usage
7. **Permissions (optional)** - Control tool access
8. **MCP (optional)** - Add custom tools

---

## Essential Reading Order

1. Start: `src/lib.rs` - Overview and re-exports
2. Then: `src/types.rs` - Understand Message and Options
3. Then: `examples/simple_query.rs` - Basic pattern
4. Then: `examples/interactive_client.rs` - Interactive pattern
5. Then: `src/client/mod.rs` - Full client implementation
6. Then: `src/transport/subprocess.rs` - CLI communication
7. Then: Add hooks from `src/hooks/mod.rs` (if needed)
8. Then: Add permissions from `src/permissions/mod.rs` (if needed)
9. Then: Add MCP from `src/mcp/` (if needed)

