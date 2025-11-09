# Claude Chat Implementation Reference

## Comprehensive Overview of claude-agent-sdk-rust

### File Structure and Organization

```
claude-agent-sdk-rust/
├── src/
│   ├── lib.rs                  # Main library entrypoint and re-exports
│   ├── client/
│   │   └── mod.rs              # ClaudeSDKClient for bidirectional communication
│   ├── query.rs                # Simple one-shot query function
│   ├── types.rs                # Type definitions (newtypes, enums, config)
│   ├── transport/
│   │   ├── mod.rs              # Transport trait definition
│   │   └── subprocess.rs       # Subprocess implementation (CLI communication)
│   ├── message/
│   │   ├── mod.rs
│   │   └── parser.rs           # JSON message parsing
│   ├── control/
│   │   ├── mod.rs
│   │   └── protocol.rs         # Control protocol (init, hooks, permissions)
│   ├── hooks/
│   │   └── mod.rs              # Hook system for event interception
│   ├── permissions/
│   │   └── mod.rs              # Permission management system
│   ├── mcp/
│   │   ├── mod.rs              # SDK MCP server integration
│   │   ├── protocol.rs         # JSONRPC protocol
│   │   ├── server.rs           # MCP server implementation
│   │   └── tool.rs             # Tool definitions
│   └── error.rs                # Error types and handling
└── examples/
    ├── simple_query.rs         # One-shot queries
    ├── interactive_client.rs   # Bidirectional communication
    ├── bidirectional_demo.rs   # Advanced bidirectional features
    ├── hooks_demo.rs           # Hook system examples
    ├── permissions_demo.rs     # Permission control examples
    └── mcp_demo.rs             # MCP integration examples
```

---

## 1. SDK Initialization and Entry Points

### Two Main Usage Patterns

#### A. Simple Queries (One-shot)
```rust
// For stateless, fire-and-forget interactions
use claude_agent_sdk::query;

let stream = query("What is 2 + 2?", None).await?;
// Returns Stream<Item = Result<Message>>
```

**Key characteristics:**
- Unidirectional communication (send all messages upfront)
- No connection management needed
- Returns a stream of messages
- Perfect for: One-off questions, batch processing, CI/CD pipelines

#### B. Interactive Client (Bidirectional)
```rust
// For stateful, interactive conversations
use claude_agent_sdk::ClaudeSDKClient;

let mut client = ClaudeSDKClient::new(options, None).await?;
client.send_message("Hello").await?;
while let Some(msg) = client.next_message().await {
    // Process messages...
}
client.close().await?;
```

**Key characteristics:**
- Bidirectional communication (send messages while reading responses)
- Stateful conversation management
- Lock-free architecture for concurrent operations
- Perfect for: Interactive chat, REPL-like interfaces, long-running sessions

### Initialization Flow

1. **Create Options**: Use `ClaudeAgentOptions::builder()` for configuration
2. **Create Client**: `ClaudeSDKClient::new(options, cli_path)` connects to CLI
3. **Spawn Background Tasks**: Multiple tokio tasks handle I/O (see below)
4. **Send/Receive Messages**: Use `send_message()` and `next_message()`
5. **Cleanup**: Call `close()` when done

---

## 2. Core Data Flow

### Message Types and Flow

The SDK handles several message types defined in `src/types.rs`:

```rust
pub enum Message {
    User { parent_tool_use_id, message, session_id },
    Assistant { parent_tool_use_id, message, session_id },
    System { subtype, data },
    Result { subtype, duration_ms, duration_api_ms, is_error, num_turns, ... },
    StreamEvent { uuid, session_id, event, parent_tool_use_id },
}

pub struct UserMessageContent {
    pub role: String,           // "user"
    pub content: Option<UserContent>,
}

pub struct AssistantMessageContent {
    pub model: String,          // e.g., "claude-3-5-sonnet"
    pub content: Vec<ContentBlock>,
}
```

### Message Content Blocks

```rust
pub enum ContentBlock {
    Text { text: String },
    Thinking { thinking: String, signature: String },
    ToolUse { id: String, name: String, input: Value },
    ToolResult { tool_use_id: String, content: Option<ContentValue>, is_error: bool },
}
```

### Message Flow Architecture

```
┌─────────────────────────────────────────────────────────┐
│                   ClaudeSDKClient                        │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  ┌──────────────────┐        ┌──────────────────┐      │
│  │  Message Reader  │        │  Control Writer  │      │
│  │  Background Task │        │  Background Task │      │
│  │                  │        │                  │      │
│  │ • Gets receiver  │        │ • Locks per-write│      │
│  │   once           │        │ • No blocking    │      │
│  │ • No lock held   │        │                  │      │
│  │   while reading  │        │                  │      │
│  └────────┬─────────┘        └────────┬─────────┘      │
│           │                           │                 │
│           │    ┌──────────────┐      │                 │
│           └───→│  Transport   │←─────┘                 │
│                │  (Arc<Mutex>)│                         │
│                └──────────────┘                         │
└─────────────────────────────────────────────────────────┘
```

---

## 3. Async and Tokio Patterns

### Background Tasks in ClaudeSDKClient

The SDK spawns **multiple background tasks** for concurrent operations:

#### Task 1: Message Reader
```rust
async fn message_reader_task(
    transport: Arc<Mutex<SubprocessTransport>>,
    protocol: Arc<Mutex<ProtocolHandler>>,
    message_tx: mpsc::UnboundedSender<Result<Message>>,
)
```
- Reads from transport once at startup (lock acquired once, then released)
- Continuously processes incoming messages
- Handles control_response, control_request, and regular messages
- Sends parsed messages to `message_rx` channel

**Key insight**: Transport lock is released immediately after getting the receiver

#### Task 2: Control Writer
```rust
async fn control_writer_task(
    transport: Arc<Mutex<SubprocessTransport>>,
    _protocol: Arc<Mutex<ProtocolHandler>>,
    mut control_rx: mpsc::UnboundedReceiver<ControlRequest>,
)
```
- Listens on control channel for outgoing messages
- Locks transport briefly for each write
- Handles: interrupt, send_message, hook_callback_response, mcp_message_response

#### Task 3: Hook Handler (if hooks configured)
```rust
async fn hook_handler_task(
    manager: Arc<Mutex<HookManager>>,
    protocol: Arc<Mutex<ProtocolHandler>>,
    mut hook_rx: mpsc::UnboundedReceiver<(String, HookEvent, Value)>,
)
```
- Processes hook events from CLI
- Invokes registered hook callbacks
- Sends responses back via control channel

#### Task 4: Hook Callback Handler (if hooks configured)
```rust
async fn hook_callback_handler_task(
    manager: Arc<Mutex<HookManager>>,
    protocol: Arc<Mutex<ProtocolHandler>>,
    control_tx: mpsc::UnboundedSender<ControlRequest>,
    mut hook_callback_rx: UnboundedReceiver<(RequestId, String, Value, Option<String>)>,
)
```
- Handles incoming hook_callback requests from CLI
- Looks up callback by ID using HookManager
- Invokes callback and sends response

#### Task 5: MCP Message Handler (if SDK MCP servers configured)
```rust
async fn mcp_message_handler_task(
    sdk_mcp_servers: HashMap<String, Arc<SdkMcpServer>>,
    protocol: Arc<Mutex<ProtocolHandler>>,
    control_tx: mpsc::UnboundedSender<ControlRequest>,
    mut mcp_callback_rx: UnboundedReceiver<(RequestId, String, Value)>,
)
```
- Handles incoming MCP requests from CLI
- Routes to appropriate SDK MCP server
- Invokes handler and sends JSONRPC response

#### Task 6: Permission Handler (if permissions configured)
```rust
async fn permission_handler_task(
    manager: Arc<Mutex<PermissionManager>>,
    protocol: Arc<Mutex<ProtocolHandler>>,
    mut permission_rx: UnboundedReceiver<(RequestId, PermissionRequest)>,
)
```
- Processes permission requests
- Invokes permission callback if configured
- Sends Allow/Deny response

### Synchronization Primitives

**Arc<Mutex<T>>** for shared state:
- `Arc<Mutex<SubprocessTransport>>` - CLI process communication
- `Arc<Mutex<ProtocolHandler>>` - Message serialization/deserialization
- `Arc<Mutex<HookManager>>` - Hook registry and invocation
- `Arc<Mutex<PermissionManager>>` - Permission checking

**mpsc::UnboundedChannel** for task communication:
- `message_rx`: Parsed messages to main client
- `control_tx`: Control requests from client/tasks
- `hook_rx`: Hook events from CLI
- `hook_callback_rx`: Hook callback requests from CLI
- `permission_rx`: Permission requests from CLI
- `mcp_callback_rx`: MCP requests from CLI

### Lock-Free Design

The client achieves **lock-free concurrent reads and writes** through:

1. **Transport receiver obtained once**: Reader task acquires transport lock once to get the receiver, then releases immediately
2. **Per-write locking**: Writer task only locks transport briefly for each write operation
3. **No contention**: Reader and writer never hold locks simultaneously
4. **Unbounded channels**: Allow producer/consumer decoupling without blocking

---

## 4. Transport and CLI Communication

### SubprocessTransport

Located in `src/transport/subprocess.rs`:

#### Initialization
```rust
pub fn new(
    prompt: PromptInput,
    options: ClaudeAgentOptions,
    cli_path: Option<PathBuf>,
) -> Result<Self>
```

**PromptInput types**:
- `PromptInput::String(String)` - Single prompt (used with `query()`)
- `PromptInput::Stream` - Stream mode (used with `ClaudeSDKClient`)

#### CLI Command Building

The SDK builds the CLI command with:

```bash
claude [mode-flags] [config-flags]
```

**Mode Flags**:
- Streaming mode (interactive): `--input-format stream-json --output-format stream-json`
- Print mode (one-shot): `--print --output-format stream-json`
- Both modes require: `--verbose` for structured output

**Configuration Flags**:
- System prompt: `--system-prompt <prompt>` or `--append-system-prompt`
- Allowed tools: `--allowedTools <tool1>,<tool2>`
- Disallowed tools: `--disallowedTools <tool1>,<tool2>`
- Max turns: `--max-turns <n>`
- Model: `--model <model>`
- Permission mode: `--permission-mode <mode>`
- Resume session: `--resume <session-id>`
- Continue conversation: `--continue`
- MCP servers: `--mcp-config <json>`
- Add directories: `--add-dir <path>`
- Session settings: `--settings <path>`

#### Security Measures

**Dangerous environment variables blocked**:
```rust
const DANGEROUS_ENV_VARS: &[&str] = &[
    "LD_PRELOAD",
    "LD_LIBRARY_PATH",
    "DYLD_INSERT_LIBRARIES",
    "DYLD_LIBRARY_PATH",
    "PATH",
    "NODE_OPTIONS",
    "PYTHONPATH",
    "PERL5LIB",
    "RUBYLIB",
];
```

**Allowed CLI flags (allowlist)**:
- timeout, retries, log-level, cache-dir

**Buffer limits**:
- Default max buffer: 1MB (configurable)
- Prevents memory exhaustion from malformed messages

#### Message Reading

```rust
fn read_messages(&mut self) -> mpsc::UnboundedReceiver<Result<serde_json::Value>>
```

Spawns background task that:
1. Reads lines from stdout
2. Parses each line as JSON
3. Validates against buffer size limit
4. Sends parsed JSON through channel
5. Handles errors gracefully

#### Message Writing

```rust
async fn write(&mut self, data: &str) -> Result<()>
```

Writes JSON messages to stdin:
- One message per line
- Automatically adds newline separator
- Handles I/O errors

---

## 5. Message Parsing

### Parse Function

Located in `src/message/parser.rs`:

```rust
pub fn parse_message(data: serde_json::Value) -> Result<Message>
```

Simple serde-based parsing:
```rust
serde_json::from_value(data.clone()).map_err(|e| {
    ClaudeError::message_parse(format!("Failed to parse message: {e}"), Some(data))
})
```

### Message Type Detection

The SDK detects message types by:
1. Checking `type` field in JSON
2. Deserializing into appropriate `Message` variant
3. Validating required fields for each type

**Special handling in reader task**:
```rust
if msg_type == Some("control_response") {
    // Handle as control response (skip in regular message stream)
    continue;
}

if msg_type == Some("control_request") {
    // Handle as control request (dispatch to appropriate handler)
    if subtype == Some("hook_callback") { /* ... */ }
    if subtype == Some("mcp_message") { /* ... */ }
    continue;
}

// Otherwise parse as regular message
```

---

## 6. Control Protocol

### Overview

Located in `src/control/protocol.rs`:

The control protocol enables bidirectional communication between SDK and CLI:

```
SDK                          CLI
 |                            |
 |--- Init Request ---------->|
 |<-- Init Response -----------|
 |                            |
 |--- User Message ---------->|
 |<-- Assistant Message ------|
 |<-- Hook Event -------------|
 |--- Hook Response --------->|
 |<-- Permission Request -----|
 |--- Permission Response --->|
 |--- Interrupt ------------->|
 |<-- Result Message ---------|
```

### Message Types

```rust
pub enum ControlMessage {
    Request(ControlRequest),      // SDK -> CLI
    Response(ControlResponse),     // CLI -> SDK
    Init(InitRequest),             // SDK -> CLI
    InitResponse(InitResponse),    // CLI -> SDK
}

pub enum ControlRequest {
    Interrupt { id: RequestId },
    SendMessage { id: RequestId, content: String },
    HookResponse { id: RequestId, hook_id: String, response: Value },
    PermissionResponse { id: RequestId, request_id: RequestId, result: PermissionResult },
    HookCallbackResponse { id: RequestId, output: Value },
    McpMessageResponse { id: RequestId, mcp_response: Value },
}
```

### Initialization

When hooks or SDK MCP servers are configured:

```rust
async fn send_initialize(
    &mut self,
    hooks_config: HashMap<HookEvent, Vec<HookMatcherConfig>>,
) -> Result<()>
```

Sends init request with:
- Hook configurations with callback IDs
- MCP server configurations

---

## 7. Error Handling

### Error Type Hierarchy

Located in `src/error.rs`:

```rust
pub enum ClaudeError {
    CliNotFound(String),              // CLI not installed
    Connection(String),               // Connection failed
    Process { message, exit_code, stderr }, // CLI process error
    JsonDecode(#[from] serde_json::Error),  // JSON parsing
    MessageParse { message, data },   // Message deserialization
    Transport(String),                // Transport layer error
    ControlProtocol(String),          // Protocol error
    Hook(String),                     // Hook execution error
    Mcp(String),                      // MCP error
    ValidationError(String),          // Schema validation
    Io(#[from] std::io::Error),       // IO error
    Timeout(String),                  // Timeout error
    InvalidConfig(String),            // Invalid configuration
}

pub type Result<T> = std::result::Result<T, ClaudeError>;
```

### Error Handling Patterns

**CLI discovery**:
```rust
fn find_cli() -> Result<PathBuf> {
    // 1. Try which::which("claude")
    // 2. Try common locations:
    //    - $HOME/.npm-global/bin/claude
    //    - /usr/local/bin/claude
    //    - $HOME/.local/bin/claude
    //    - $HOME/node_modules/.bin/claude
    //    - $HOME/.yarn/bin/claude
    // 3. Return helpful error with installation instructions
}
```

**Message parsing**:
- Includes raw data in error context for debugging
- Non-fatal errors (unknown messages) are logged but don't crash

**Process errors**:
- Captures exit code and stderr output
- Provides context for debugging

---

## 8. Hooks System

### Overview

Located in `src/hooks/mod.rs`:

Allows intercepting and modifying agent behavior at various lifecycle points:

```rust
pub enum HookEvent {
    PreToolUse,           // Before tool execution
    PostToolUse,          // After tool execution
    UserPromptSubmit,     // When user submits prompt
    Stop,                 // When conversation stops
    SubagentStop,         // When subagent stops
    PreCompact,           // Before conversation compacting
}
```

### Hook Matching

```rust
pub struct HookMatcher {
    pub matcher: Option<String>,  // Tool name pattern (e.g., "Bash", "Write|Edit")
    pub hooks: Vec<HookCallback>, // List of callbacks for matching
}
```

Pattern matching:
- `None` - matches all tools
- `"Bash"` - matches specific tool
- `"Write|Edit"` - matches multiple tools

### Hook Callback Type

```rust
pub type HookCallback = Arc<
    dyn Fn(
        serde_json::Value,        // Event data
        Option<String>,           // Tool name
        HookContext,              // Execution context
    ) -> Pin<Box<dyn Future<Output = Result<HookOutput>> + Send>>
        + Send
        + Sync,
>;
```

### Hook Output

```rust
pub struct HookOutput {
    pub decision: Option<HookDecision>,          // Block or allow
    pub system_message: Option<String>,          // Add system message
    pub hook_specific_output: Option<Value>,     // Custom data
}

pub enum HookDecision {
    Block,  // Prevent the action
}
```

### Callback ID Management

The HookManager generates unique callback IDs:
- Format: `hook_N` (e.g., `hook_0`, `hook_1`)
- Maps callback_id -> (matcher_index, callback_index)
- Used by CLI to route hook_callback requests back to SDK

```rust
pub fn register_with_ids(&mut self, matcher: HookMatcher) -> Vec<String> {
    // Returns vector of callback IDs for hooks in this matcher
    // These IDs are sent to CLI during initialization
}

pub async fn invoke_by_id(
    &self,
    callback_id: &str,
    event_data: serde_json::Value,
    tool_name: Option<String>,
    context: HookContext,
) -> Result<HookOutput> {
    // CLI calls this via hook_callback request with callback_id
}
```

---

## 9. Permissions System

### Overview

Located in `src/permissions/mod.rs`:

Controls which tools Claude can use and with what parameters:

```rust
pub struct PermissionManager {
    callback: Option<CanUseToolCallback>,
    allowed_tools: Option<Vec<ToolName>>,  // None = all allowed
    disallowed_tools: Vec<ToolName>,
}
```

### Permission Request

```rust
pub struct PermissionRequest {
    pub tool_name: ToolName,
    pub tool_input: serde_json::Value,
    pub context: ToolPermissionContext,
}

pub struct ToolPermissionContext {
    pub suggestions: Vec<PermissionUpdate>,
}
```

### Permission Result

```rust
pub enum PermissionResult {
    Allow(PermissionResultAllow),
    Deny(PermissionResultDeny),
}

pub struct PermissionResultAllow {
    pub updated_input: Option<Value>,
    pub updated_permissions: Option<Vec<PermissionUpdate>>,
}

pub struct PermissionResultDeny {
    pub message: String,
    pub interrupt: bool,
}
```

### Permission Callback

```rust
pub type CanUseToolCallback = Arc<
    dyn Fn(
        ToolName,
        serde_json::Value,
        ToolPermissionContext,
    ) -> Pin<Box<dyn Future<Output = Result<PermissionResult>> + Send>>
        + Send
        + Sync,
>;
```

### Permission Checking Flow

```rust
pub async fn can_use_tool(
    &self,
    tool_name: ToolName,
    tool_input: serde_json::Value,
    context: ToolPermissionContext,
) -> Result<PermissionResult> {
    // 1. Check disallowed_tools (deny if found)
    // 2. Check allowed_tools if set (deny if not in list)
    // 3. Invoke callback if set
    // 4. Default to Allow if no restrictions
}
```

---

## 10. MCP (Model Context Protocol) Integration

### Overview

Located in `src/mcp/`:

Two types of MCP servers supported:

1. **External MCP Servers** - Subprocess-based (stdio, SSE, HTTP)
2. **SDK MCP Servers** - In-process, implemented in Rust

### SDK MCP Server Benefits

- No subprocess overhead
- Same process execution
- Direct function calls
- Full stack traces for debugging
- Type-safe tool signatures

### Tool Definition

```rust
pub struct SdkMcpTool {
    name: String,
    description: String,
    input_schema: Value,  // JSON Schema
    handler: ToolHandler, // Async function
}

impl SdkMcpTool {
    pub fn new<F, Fut>(
        name: &str,
        description: &str,
        input_schema: Value,
        handler: F,
    ) -> Self
    where
        F: Fn(serde_json::Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<ToolResult>> + Send + 'static,
    { }
}

pub type ToolHandler = Arc<
    dyn Fn(serde_json::Value) -> Pin<Box<dyn Future<Output = Result<ToolResult>> + Send>>
        + Send
        + Sync,
>;
```

### Tool Result

```rust
pub struct ToolResult {
    pub content: Vec<ToolContent>,
    pub is_error: Option<bool>,
}

pub enum ToolContent {
    Text { text: String },
    Image { data: String, media_type: String },
}
```

### MCP Server

```rust
pub struct SdkMcpServer {
    name: String,
    version: String,
    tools: HashMap<String, Arc<SdkMcpTool>>,
}

impl SdkMcpServer {
    pub fn new(name: &str) -> Self { }
    pub fn version(mut self, version: &str) -> Self { }
    pub fn tool(mut self, tool: SdkMcpTool) -> Self { }
    
    pub async fn handle_request(
        &self,
        request: JsonRpcRequest,
    ) -> Result<JsonRpcResponse> { }
}
```

### JSONRPC Protocol

```rust
pub struct JsonRpcRequest {
    pub jsonrpc: String,      // "2.0"
    pub method: String,       // "tools/list" or "tools/call"
    pub params: Value,
    pub id: Value,
}

pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub result: Option<Value>,
    pub error: Option<McpError>,
    pub id: Value,
}

pub struct McpError {
    pub code: i32,
    pub message: String,
}
```

### Integration with ClaudeSDKClient

When SDK MCP servers are registered:

1. Add to `ClaudeAgentOptions` via builder
2. SDK extracts them during client initialization
3. Stores as `Arc<SdkMcpServer>` in HashMap
4. Spawns mcp_message_handler_task
5. When CLI sends mcp_message request:
   - Extract server_name and JSONRPC request
   - Look up server in HashMap
   - Call `server.handle_request()`
   - Send JSONRPC response back to CLI

---

## 11. Configuration and Options

### ClaudeAgentOptions

Comprehensive configuration via builder pattern:

```rust
pub struct ClaudeAgentOptions {
    // Tools
    pub allowed_tools: Vec<ToolName>,
    pub disallowed_tools: Vec<ToolName>,
    
    // System prompt
    pub system_prompt: Option<SystemPrompt>,
    
    // MCP servers
    pub mcp_servers: McpServers,
    
    // Permissions and modes
    pub permission_mode: Option<PermissionMode>,
    pub can_use_tool: Option<CanUseToolCallback>,
    
    // Session management
    pub continue_conversation: bool,
    pub resume: Option<SessionId>,
    pub fork_session: bool,
    
    // Limits
    pub max_turns: Option<u32>,
    pub max_buffer_size: Option<usize>,
    
    // Model and features
    pub model: Option<String>,
    pub include_partial_messages: bool,
    
    // Execution context
    pub cwd: Option<PathBuf>,
    pub env: HashMap<String, String>,
    pub add_dirs: Vec<PathBuf>,
    
    // Configuration
    pub settings: Option<PathBuf>,
    pub setting_sources: Option<Vec<SettingSource>>,
    
    // Callbacks and hooks
    pub hooks: Option<HashMap<HookEvent, Vec<HookMatcher>>>,
    
    // Custom agents
    pub agents: Option<HashMap<String, AgentDefinition>>,
    
    // CLI customization
    pub extra_args: HashMap<String, Option<String>>,
    pub permission_prompt_tool_name: Option<String>,
    pub user: Option<String>,
}
```

### Builder Pattern

```rust
let options = ClaudeAgentOptions::builder()
    .system_prompt("You are helpful")
    .max_turns(5)
    .add_allowed_tool("Read")
    .add_allowed_tool("Bash")
    .build();
```

---

## 12. Important Implementation Details

### Gotchas and Edge Cases

1. **Transport Receiver Lifetime**
   - The receiver returned by `read_messages()` is owned (no lifetime issues)
   - Reader task acquires lock once, then releases immediately
   - Allows concurrent writes without blocking reads

2. **Message Type Discrimination**
   - Control messages (control_response, control_request) are intercepted before regular parsing
   - These don't appear in the user's message stream
   - Only User/Assistant/Result/System/StreamEvent messages reach the application

3. **Hook Callback ID Generation**
   - IDs must match between SDK and CLI initialization
   - Format: `hook_N` where N is incremented sequentially
   - Used to route incoming hook_callback requests from CLI back to correct callback

4. **Permission Default Behavior**
   - Without explicit allowed_tools list: allows all tools
   - With allowed_tools list: only allows listed tools
   - Disallowed_tools is checked first (deny wins)

5. **Streaming vs Non-streaming Mode**
   - Streaming mode: `--input-format stream-json --output-format stream-json`
   - Used when: `PromptInput::Stream` OR hooks/permissions/SDK MCP configured
   - Print mode: `--print --output-format stream-json`
   - Used for: Simple one-shot queries with no control protocol

6. **Interrupt Support**
   - Interrupt message is sent correctly by SDK
   - CLI may not process control messages in all versions
   - Method exists for architecture compatibility, but behavior varies

7. **Lock Contention**
   - Reader task holds lock only to get receiver channel
   - Writer task locks only during actual write
   - No contention because reader releases immediately

8. **Channel Closures**
   - When sender side is dropped, receiver gets None and exits loop
   - Background tasks automatically exit when channels close
   - Client drop() calls drop handlers for Arcs, which releases resources

---

## 13. Example Usage Patterns

### Pattern 1: Simple Query
```rust
let options = ClaudeAgentOptions::builder()
    .max_turns(1)
    .build();

let stream = query("Translate to French: Hello", Some(options)).await?;
let mut stream = Box::pin(stream);

while let Some(message) = stream.next().await {
    match message? {
        Message::Assistant { message, .. } => {
            for block in &message.content {
                if let ContentBlock::Text { text } = block {
                    println!("{}", text);
                }
            }
        }
        Message::Result { .. } => break,
        _ => {}
    }
}
```

### Pattern 2: Interactive Chat
```rust
let mut client = ClaudeSDKClient::new(ClaudeAgentOptions::default(), None).await?;

loop {
    let input = read_line_from_user();
    client.send_message(input).await?;
    
    while let Some(message) = client.next_message().await {
        match message? {
            Message::Assistant { message, .. } => display_response(&message),
            Message::Result { .. } => break,
            _ => {}
        }
    }
}

client.close().await?;
```

### Pattern 3: With Hooks
```rust
let hook = HookManager::callback(|event_data, tool_name, _ctx| async move {
    println!("Tool {:?} was called", tool_name);
    Ok(HookOutput::default())
});

let matcher = HookMatcherBuilder::new(Some("Bash"))
    .add_hook(hook)
    .build();

let mut hooks = HashMap::new();
hooks.insert(HookEvent::PreToolUse, vec![matcher]);

let options = ClaudeAgentOptions::builder()
    .hooks(hooks)
    .build();

let stream = query("Run ls command", Some(options)).await?;
```

### Pattern 4: With Permissions
```rust
let permission_callback = PermissionManager::callback(
    |tool_name, _tool_input, _context| async move {
        match tool_name.as_str() {
            "Read" | "Glob" => Ok(PermissionResult::Allow(
                PermissionResultAllow {
                    updated_input: None,
                    updated_permissions: None,
                }
            )),
            "Bash" => Ok(PermissionResult::Deny(
                PermissionResultDeny {
                    message: "Bash not allowed".to_string(),
                    interrupt: true,
                }
            )),
            _ => Ok(PermissionResult::Allow(PermissionResultAllow {
                updated_input: None,
                updated_permissions: None,
            }))
        }
    }
);

let options = ClaudeAgentOptions::builder()
    .can_use_tool(permission_callback)
    .build();
```

### Pattern 5: With SDK MCP Server
```rust
let calculator_tool = SdkMcpTool::new(
    "add",
    "Add two numbers",
    json!({
        "type": "object",
        "properties": {
            "a": {"type": "number"},
            "b": {"type": "number"}
        },
        "required": ["a", "b"]
    }),
    |input| Box::pin(async move {
        let a = input["a"].as_f64().unwrap_or(0.0);
        let b = input["b"].as_f64().unwrap_or(0.0);
        Ok(ToolResult {
            content: vec![ToolContent::Text {
                text: format!("{} + {} = {}", a, b, a + b),
            }],
            is_error: None,
        })
    }),
);

let server = SdkMcpServer::new("calc")
    .version("1.0.0")
    .tool(calculator_tool);

let mut servers = HashMap::new();
servers.insert("calc", McpServerConfig::Sdk(
    SdkMcpServerMarker {
        name: "calc".to_string(),
        instance: Arc::new(server),
    }
));

let options = ClaudeAgentOptions::builder()
    .mcp_servers(servers)
    .build();

let stream = query("What is 5 + 3?", Some(options)).await?;
```

---

## 14. Key Takeaways for chat-ui Integration

1. **Two Entry Points**: Use `query()` for simple one-offs, `ClaudeSDKClient` for interactive chat

2. **Message Streaming**: All communication is stream-based via channels and async iterators

3. **Background Tasks**: SDK manages multiple background tasks automatically, no manual coordination needed

4. **Lock-Free Design**: Reader and writer don't block each other; safe for concurrent operations

5. **Protocol Flexibility**: Control protocol (hooks, permissions, MCP) is optional and self-contained

6. **Error Context**: Errors include relevant data (exit codes, stderr, raw JSON) for debugging

7. **Configuration via Builder**: Flexible, type-safe configuration with sensible defaults

8. **Extensibility**: Hooks and permissions allow custom behavior without modifying SDK

9. **In-Process Tools**: SDK MCP servers enable custom tools without subprocess management

10. **Tokio Foundation**: All async operations use tokio, compatible with tokio-based applications

